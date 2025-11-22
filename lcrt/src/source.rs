use std::{pin::Pin, sync::Arc};

use rustc_hash::{FxHashMap, FxHashSet};
use tokio::{select, sync::mpsc, time};

use crate::{
    Address, BUFFER_LEN,
    message::{self, Message},
    node::{LCRTNode, NodeInfo},
};

#[derive(Clone, Copy, Debug)]
struct ConstructionNode {
    hop_distance: u16,
    position: glam::DVec3,
    radius: f64,
    // index: petgraph::graph::NodeIndex,
    coverage_index: petgraph::graph::NodeIndex,
}

enum State<NA> {
    Startup,
    Construction {
        nodes: FxHashMap<NA, ConstructionNode>,
        // network: petgraph::graph::Graph<(), ()>,
        coverage: petgraph::graph::Graph<NA, ()>, // TODO: convert to CSR?
        timeout: Pin<Box<time::Sleep>>,
    },
    Streaming {
        nodes: FxHashMap<NA, message::NodeData>,
        network: petgraph::graph::Graph<(), ()>, // TODO: convert to CSR?
        neighbours: Vec<NA>,
    },
}

struct Source<N, NA, GA> {
    node: Arc<LCRTNode<N, NA, GA>>,
    rx: mpsc::Receiver<Message<NA, GA>>,
    state: State<NA>,
}

pub type SourceHandle<NA, GA> = mpsc::Sender<Message<NA, GA>>;

pub fn spawn<N, NA, GA>(n: Arc<LCRTNode<N, NA, GA>>) -> SourceHandle<NA, GA>
where
    N: NodeInfo,
    NA: Address,
    GA: Address,
{
    let (tx, rx) = mpsc::channel(BUFFER_LEN);

    let mut s = Source {
        node: n,
        rx,
        state: State::Startup,
    };

    tokio::spawn(async move {
        loop {
            s.step().await;
        }
    }); // TODO: capture this handle?

    tx
}

impl<N, NA, GA> Source<N, NA, GA>
where
    N: NodeInfo,
    NA: Address,
    GA: Address,
{
    async fn step(&mut self) {
        if matches!(self.state, State::Startup) {
            self.construct().await;
            return;
        }

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

    async fn construct(&mut self) {
        let k = self.node.config.k;

        let position = self.node.info.position().await;
        let radius = self.node.info.coverage_radius();

        self.node
            .tx(message::AreaConstruction {
                area: self.node.address,
                ttl: k,
                k,
                position,
                radius,
            })
            .await
            .unwrap();

        let mut nodes = FxHashMap::default();
        // let mut network = petgraph::Graph::new();
        let mut coverage = petgraph::Graph::new();

        // add the source to the network and nodes map
        // let index = network.add_node(());
        let coverage_index = coverage.add_node(self.node.address);
        nodes.insert(
            self.node.address,
            ConstructionNode {
                hop_distance: 0,
                position,
                radius,
                // index,
                coverage_index,
            },
        );

        self.state = State::Construction {
            nodes,
            // network,
            coverage,
            timeout: Box::pin(time::sleep(self.node.config.source_construct_timeout)),
        };
    }

    async fn handle(&mut self, m: Message<NA, GA>) {
        match m {
            Message::AreaConstruction(_) | Message::AreaInfo(_) | Message::Data(_) => {
                // TODO: verify consistency
            }

            Message::JoinReport(join_report) => self.handle_join_report(join_report).await,
        }
    }

    async fn handle_timeout(&mut self) {
        match &mut self.state {
            State::Construction {
                nodes,
                // network,
                coverage,
                timeout: _,
            } => {
                let mut network = petgraph::Graph::with_capacity(nodes.len(), 0);
                let new_nodes: FxHashMap<NA, message::NodeData> = nodes
                    .iter()
                    .map(|(a, n)| {
                        (
                            *a,
                            message::NodeData {
                                position: n.position,
                                radius: n.radius,
                                index: network.add_node(()),
                            },
                        )
                    })
                    .collect();

                let levels = nodes.values().map(|n| n.hop_distance).max().unwrap();
                debug_assert_ne!(levels, 0);

                // TODO: use rayon?

                fn extract_level<NA>(
                    set: &mut FxHashSet<NA>,
                    nodes: &FxHashMap<NA, ConstructionNode>,
                    level: u16,
                ) where
                    NA: Address,
                {
                    set.extend(
                        nodes
                            .iter()
                            .filter(|(_, n)| n.hop_distance == level)
                            .map(|(a, _)| *a),
                    );
                }

                let mut l = levels - 1;
                let mut uncovered = FxHashSet::default();
                let mut potential_forwarders = FxHashSet::default();
                while l > 0 {
                    extract_level(&mut uncovered, nodes, l + 1);
                    extract_level(&mut potential_forwarders, nodes, l);

                    while !uncovered.is_empty() {
                        let Some((fa, _)) = potential_forwarders
                            .iter()
                            .map(|a| (a, nodes[a].eta()))
                            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                        else {
                            // TODO: abandon uncovered nodes?
                            // If nodes are removed from the graph, switch to a StableGraph.
                            todo!("deal with failed construction");
                        };

                        // TODO: add index to ConstructionNode to avoid double lookup?
                        let forwarder_index = new_nodes[fa].index;

                        for child in coverage
                            .neighbors(forwarder_index)
                            .map(|ni| coverage[ni])
                            .filter(|a| uncovered.remove(a))
                        {
                            network.add_edge(forwarder_index, new_nodes[&child].index, ());
                        }
                    }

                    l += 1;
                    potential_forwarders.clear();
                }

                // TODO: set NA as the node weight to avoid extra n complexity?
                let neighbours = network
                    .neighbors(new_nodes[&self.node.address].index)
                    .map(|i| {
                        new_nodes
                            .iter()
                            .find(|(_, n)| n.index == i)
                            .map(|(a, _)| *a)
                            .unwrap()
                    })
                    .collect();

                // neither Default impl allocates memory, so take is cheap
                // let nodes: FxHashMap<NA, message::NodeData> = mem::take(nodes)
                //     .into_iter()
                //     .map(|(a, n)| (a, n.into()))
                //     .collect();
                // let network = mem::take(network);

                self.node
                    .tx(message::AreaInfo {
                        area: self.node.address,
                        network: network.clone(),
                        // nodes: nodes.clone(),
                        nodes: new_nodes.clone(),
                    })
                    .await
                    .unwrap();

                self.state = State::Streaming {
                    nodes: new_nodes,
                    network,
                    neighbours,
                };
            }

            _ => todo!(),
        }
    }

    async fn handle_join_report(&mut self, m: message::JoinReport<NA>) {
        match &mut self.state {
            State::Startup => {
                todo!("can't be for our area as we haven't sent the construction message yet")
            }

            State::Construction {
                nodes,
                // network,
                coverage,
                timeout,
            } => {
                timeout
                    .as_mut()
                    .reset(time::Instant::now() + self.node.config.construct_timeout);

                // deduplicate
                if nodes.contains_key(&m.address) {
                    return;
                }

                // let index = network.add_node(());
                let coverage_index = coverage.add_node(m.address);

                let node = ConstructionNode {
                    hop_distance: m.hop_distance,
                    position: m.position,
                    radius: m.radius,
                    // index,
                    coverage_index,
                };

                let potential_forwarders = nodes
                    .values()
                    .filter(|n| n.hop_distance == m.hop_distance - 1)
                    .map(|f| (f, &node));
                let potential_children = nodes
                    .values()
                    .filter(|n| n.hop_distance == m.hop_distance + 1)
                    .map(|c| (&node, c));

                // filter the candidates by coverage and add edges to the graph
                for (f, c) in potential_forwarders
                    .chain(potential_children)
                    .filter(|(f, c)| f.covers(c))
                {
                    coverage.add_edge(f.coverage_index, c.coverage_index, ());
                }

                nodes.insert(m.address, node);
            }

            State::Streaming {
                nodes,
                network,
                neighbours,
            } => todo!("emit warning"),
        }
    }
}

impl ConstructionNode {
    fn eta(&self) -> f32 {
        // TODO: take account of the outgoing edges in the network
        todo!()
    }

    fn covers(&self, other: &Self) -> bool {
        // TODO: just use radius of the potential forwarder rather than min?
        let min_radius = self.radius.min(other.radius);
        self.position.distance_squared(other.position) <= min_radius * min_radius
    }
}
