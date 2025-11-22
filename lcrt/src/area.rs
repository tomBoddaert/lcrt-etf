use std::{mem, num::NonZero, pin::Pin, sync::Arc};

use rustc_hash::{FxHashMap, FxHashSet};
use tokio::{select, sync::mpsc, time};

use crate::{
    Address, BUFFER_LEN,
    message::{self, Message},
    node::{LCRTNode, NodeInfo},
};

enum State<NA> {
    Startup,
    Construction {
        min_hop_distance: u16,
        position: glam::DVec3,
        radius: f64,
        joins_forwarded: FxHashSet<NA>,
        timeout: Pin<Box<time::Sleep>>,
    },
    AwaitingAreaInfo {
        hop_distance: u16,
        joins_forwarded: FxHashSet<NA>,
    },
    Streaming {
        hop_distance: u16,
        nodes: FxHashMap<NA, message::NodeData>,
        network: petgraph::graph::Graph<(), ()>, // TODO: convert to CSR?
        neighbours: Vec<NA>,
    },
}

struct Area<N, NA, GA> {
    area_id: NA,
    node: Arc<LCRTNode<N, NA, GA>>,
    rx: mpsc::Receiver<Message<NA, GA>>,
    state: State<NA>,
}

pub type AreaHandle<NA, GA> = mpsc::Sender<Message<NA, GA>>;

pub fn spawn<N, NA, GA>(n: Arc<LCRTNode<N, NA, GA>>, id: NA) -> AreaHandle<NA, GA>
where
    N: NodeInfo,
    NA: Address,
    GA: Address,
{
    let (tx, rx) = mpsc::channel(BUFFER_LEN);

    let mut g = Area {
        area_id: id,
        node: n,
        rx,
        state: State::Startup,
    };

    tokio::spawn(async move {
        loop {
            g.step().await;
        }
    }); // TODO: capture this handle?

    tx
}

impl<N, NA, GA> Area<N, NA, GA>
where
    N: NodeInfo,
    NA: Address,
    GA: Address,
{
    async fn step(&mut self) {
        let timeout = async {
            if let State::Construction { timeout, .. } = &mut self.state {
                timeout.await
            } else {
                std::future::pending().await
            }
        };

        select! { biased;
            () = timeout => {
                self.handle_timeout().await
            },

            m = self.rx.recv() => match m {
                Some(m) => self.handle(m).await,
                None => todo!(),
            },
        };
    }

    async fn handle(&mut self, m: Message<NA, GA>) {
        // TODO: verify m.address == self.address?
        // or remove m.address to remove redundancy?

        match m {
            Message::AreaConstruction(m) => self.handle_area_construction(m).await,
            Message::JoinReport(m) => self.handle_join_report(m).await,
            Message::AreaInfo(m) => self.handle_area_info(m).await,
            Message::Data(m) => self.handle_data(m).await,
        }
    }

    async fn handle_timeout(&mut self) {
        match &mut self.state {
            State::Construction {
                min_hop_distance,
                position,
                radius,
                joins_forwarded,
                timeout: _,
            } => {
                let hop_distance = *min_hop_distance;

                self.node
                    .tx(message::JoinReport {
                        area: self.area_id,
                        address: self.node.address,
                        hop_distance,
                        position: *position,
                        radius: *radius,
                        forwarder_hop_distance: *min_hop_distance,
                    })
                    .await
                    .unwrap();

                self.state = State::AwaitingAreaInfo {
                    hop_distance,
                    joins_forwarded: mem::take(joins_forwarded), // the Default impl does no allocation, so take is "free"
                };
            }

            _ => todo!("error? (maybe only in debug)"),
        }
    }

    async fn handle_area_construction(&mut self, m: message::AreaConstruction<NA>) {
        match &mut self.state {
            State::Startup => {
                let position = self.node.info.position().await;
                let radius = self.node.info.coverage_radius();

                let min_radius = radius.min(m.radius);
                // if either node is outside of the other's RTR, ignore it
                if position.distance_squared(m.position) > min_radius * min_radius {
                    return;
                }

                let ttl = m.ttl.get() - 1;
                debug_assert_ne!(m.k.get(), ttl); // TODO: use NonZero for hop_distance?

                self.state = State::Construction {
                    min_hop_distance: m.k.get() - ttl,
                    position,
                    radius,
                    joins_forwarded: FxHashSet::default(),
                    timeout: Box::pin(time::sleep(self.node.config.construct_timeout)),
                };

                if let Some(ttl) = NonZero::new(ttl) {
                    self.node
                        .tx(message::AreaConstruction {
                            ttl,
                            // address: self.node.address,
                            position,
                            radius,
                            ..m
                        })
                        .await
                        .unwrap();
                }
            }

            State::Construction {
                min_hop_distance,
                position,
                radius,
                joins_forwarded: _,
                timeout,
            } => {
                let min_radius = radius.min(m.radius);
                // if either node is outside of the other's RTR, ignore it
                if position.distance_squared(m.position) > min_radius * min_radius {
                    return;
                }

                timeout
                    .as_mut()
                    .reset(time::Instant::now() + self.node.config.construct_timeout);

                let ttl = m.ttl.get() - 1;
                let hop_distance = m.k.get() - ttl;
                debug_assert_ne!(hop_distance, 0); // TODO: use NonZero?

                // if the ttl is no better, ignore it
                if hop_distance >= *min_hop_distance {
                    return;
                }
                *min_hop_distance = hop_distance;

                // TODO: handle error
                // assuming k has stayed constant, hd < mhd, so ttl > maxttl >= 0
                // if this fails, then k must have changed
                let ttl = NonZero::new(ttl).unwrap();

                self.node
                    .tx(message::AreaConstruction {
                        ttl,
                        position: *position,
                        radius: *radius,
                        ..m
                    })
                    .await
                    .unwrap();
            }

            _ => todo!("handle error"),
        }
    }

    async fn handle_join_report(&mut self, m: message::JoinReport<NA>) {
        fn check_hop_and_forward<N, NA, GA>(
            node: &LCRTNode<N, NA, GA>,
            forwarded: &mut FxHashSet<NA>,
            hop_distance: u16,
            mut m: message::JoinReport<NA>,
        ) -> Option<impl Future<Output = Result<(), mpsc::error::SendError<Message<NA, GA>>>>>
        where
            N: NodeInfo,
            NA: Address,
            GA: Address,
        {
            // TODO: ensure that we are in eachother's RTRs?

            // only send the message towards the source and deduplicate
            if hop_distance >= m.forwarder_hop_distance || forwarded.contains(&node.address) {
                return None;
            }

            forwarded.insert(node.address);

            m.forwarder_hop_distance = hop_distance;
            Some(node.tx(m))
        }

        match &mut self.state {
            State::Startup => todo!("cache them here"),

            State::Construction {
                min_hop_distance,
                joins_forwarded,
                ..
            } => {
                if let Some(future) =
                    check_hop_and_forward(&self.node, joins_forwarded, *min_hop_distance, m)
                {
                    future.await.unwrap();
                }
            }

            State::AwaitingAreaInfo {
                hop_distance,
                joins_forwarded,
            } => {
                if let Some(future) =
                    check_hop_and_forward(&self.node, joins_forwarded, *hop_distance, m)
                {
                    future.await.unwrap();
                }
            }

            State::Streaming { .. } => {
                // TODO: emit a warning
            }
        }
    }

    async fn handle_area_info(&mut self, mut m: message::AreaInfo<NA>) {
        match &mut self.state {
            State::AwaitingAreaInfo {
                hop_distance,
                joins_forwarded: _,
            } => {
                // TODO: set NA as the node weight?
                let neighbours: Vec<NA> = m
                    .network
                    .neighbors(m.nodes[&self.node.address].index)
                    .map(|i| {
                        m.nodes
                            .iter()
                            .find(|(_, n)| n.index == i)
                            .map(|(a, _)| *a)
                            .unwrap()
                    })
                    .collect();

                let network = m.network;
                let nodes = m.nodes;

                if !neighbours.is_empty() {
                    // TODO: avoid clone by transmitting by reference?
                    // Would require switching the channel out for an async call.
                    // Data needs to be serialised anyway, so it doesn't need to be owned.
                    m.network = network.clone();
                    m.nodes = nodes.clone();

                    self.node.tx(m).await.unwrap();
                }

                self.state = State::Streaming {
                    hop_distance: *hop_distance,
                    nodes,
                    network,
                    neighbours,
                }
            }

            State::Streaming { .. } => {
                // TODO: any reason not to ignore it here?
                // It is most likely a repeat.
            }

            _ => todo!(),
        }
    }

    async fn handle_data(&mut self, m: message::Data<NA, GA, Box<[u8]>>) {
        match &mut self.state {
            State::Startup | State::Construction { .. } | State::AwaitingAreaInfo { .. } => {
                todo!("cache for later")
            }

            State::Streaming { neighbours, .. } => {
                if neighbours.is_empty() {
                    return;
                }

                // TODO: expose neighbours somehow
                self.node.tx(m).await.unwrap();
            }
        }
    }
}
