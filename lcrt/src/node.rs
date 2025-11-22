use std::{collections::hash_map, net::Ipv4Addr, sync::Arc};

use rustc_hash::FxHashMap;
use tokio::sync::{Mutex, mpsc};

use crate::{
    Address, BUFFER_LEN, Config,
    area::{self, AreaHandle},
    message::{self, Message},
    source::{self, SourceHandle},
};

pub trait NodeInfo: Send + Sync + 'static {
    fn position(&self) -> impl Future<Output = glam::DVec3> + Send;
    fn coverage_radius(&self) -> f64;

    fn output_bitrate(&self) -> impl Future<Output = f32>;
}

// pub trait NodeMessages {
//     type SendError;

//     async fn send(&mut self, m: Message) -> Result<(), Self::SendError>;
//     async fn recv(&mut self) -> Option<Message>;
// }

// pub struct MpscMessages {
//     rx: message::Receiver,
//     tx: message::Sender,
// }

pub(crate) struct LCRTNode<N, NA = Ipv4Addr, GA = Ipv4Addr> {
    pub info: N,
    pub address: NA,
    pub config: Config,
    pub areas: Mutex<FxHashMap<NA, AreaHandle<NA, GA>>>,
    pub source: Mutex<Option<SourceHandle<NA, GA>>>,
    pub tx: message::Sender<NA, GA>,
}

pub struct Node<N, NA, GA> {
    node: Arc<LCRTNode<N, NA, GA>>,
    task: tokio::task::JoinHandle<()>,
    // rx: message::Receiver<NA, GA>,
}

impl<N, NA, GA> Node<N, NA, GA>
where
    N: NodeInfo,
    NA: Address,
    GA: Address,
{
    pub fn spawn(
        node: N,
        address: NA,
        config: Config,
    ) -> (Self, message::Sender<NA, GA>, message::Receiver<NA, GA>) {
        let (in_tx, mut rx) = mpsc::channel(BUFFER_LEN);
        let (tx, out_rx) = mpsc::channel(BUFFER_LEN);

        let node = Arc::new(LCRTNode {
            info: node,
            address,
            config,
            areas: Mutex::new(FxHashMap::default()),
            source: Mutex::default(),
            tx,
        });

        let n = node.clone();
        let task = tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                n.handle(m).await;
            }
        });

        (Self { node, task }, in_tx, out_rx)
    }

    pub async fn construct_area(&mut self) {
        let mut source = self.node.source.lock().await;

        if source.is_some() {
            return;
        }

        *source = Some(source::spawn(self.node.clone()));

        // let entry = match sources.entry(address) {
        //     hash_map::Entry::Occupied(_occupied_entry) => return,
        //     hash_map::Entry::Vacant(vacant_entry) => vacant_entry,
        // };

        // entry.insert(source::spawn(self.node.clone(), address));
    }

    #[inline]
    pub fn is_shutdown(&self) -> bool {
        self.task.is_finished()
    }

    // pub async fn shutdown(&mut self) {
    //     self.node.
    // }

    // pub fn new(
    //     node: N,
    //     address: NA,
    //     config: Config,
    // ) -> (
    //     Self,
    //     message::Sender<NA, GA>,
    //     message::Receiver<NA, GA>,
    // ) {
    //     let (in_tx, rx) = mpsc::channel(BUFFER_LEN);
    //     let (tx, out_rx) = mpsc::channel(BUFFER_LEN);

    //     let node = Arc::new(LCRTNode {
    //         info: node,
    //         address,
    //         config,
    //         groups: Mutex::new(FxHashMap::default()),
    //         sources: Mutex::new(FxHashMap::default()),
    //         tx,
    //     });

    //     let n = node.clone();
    //     tokio::spawn(async move {
    //         loop {
    //             n.step().await;
    //         }
    //     });

    //     (Self { node, rx }, in_tx, out_rx)
    // }

    // pub async fn step(&mut self) -> bool {
    //     let Some(m) = self.rx.recv().await else {
    //         return false;
    //     };

    //     let address = m.group();
    //     let mut groups = self.node.groups.lock().await;

    //     let entry = match groups.entry(address) {
    //         hash_map::Entry::Occupied(occupied_entry) => occupied_entry,

    //         hash_map::Entry::Vacant(vacant_entry) if matches!(m, Message::AreaConstruction(_)) => {
    //             vacant_entry.insert_entry(group::spawn(self.node.clone(), address))
    //         }

    //         _ => {
    //             todo!("log error message and return true");
    //         }
    //     };

    //     if let Err(_err) = entry.get().send(m).await {
    //         entry.remove();
    //         // TODO: log err
    //         return false;
    //     }

    //     true
    // }

    // pub fn run(mut self) {
    //     tokio::spawn(async move { while self.step().await {} });
    // }
}

impl<N, NA, GA> LCRTNode<N, NA, GA>
where
    N: NodeInfo,
    NA: Address,
    GA: Address,
{
    #[inline]
    pub async fn tx<M>(&self, m: M) -> Result<(), mpsc::error::SendError<Message<NA, GA>>>
    where
        M: Into<Message<NA, GA>>,
    {
        self.tx.send(m.into()).await
    }

    async fn handle(self: &Arc<Self>, m: Message<NA, GA>) {
        let address = m.area();
        let mut areas = self.areas.lock().await;

        let entry = match areas.entry(address) {
            hash_map::Entry::Occupied(occupied_entry) => occupied_entry,

            hash_map::Entry::Vacant(vacant_entry) if matches!(m, Message::AreaConstruction(_)) => {
                vacant_entry.insert_entry(area::spawn(self.clone(), address))
            }

            _ => {
                todo!("log error message and return true");
            }
        };

        if let Err(_err) = entry.get().send(m).await {
            entry.remove();
            // TODO: log err
        }
    }
}
