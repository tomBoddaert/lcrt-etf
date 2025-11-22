use std::{net::Ipv4Addr, num::NonZero};

use petgraph::Graph;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AreaConstruction<NA> {
    pub area: NA,
    pub ttl: NonZero<u16>,
    pub k: NonZero<u16>,
    pub position: glam::DVec3,
    pub radius: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinReport<NA> {
    pub area: NA,
    pub address: NA,
    pub hop_distance: u16,
    pub position: glam::DVec3,
    pub radius: f64,
    pub forwarder_hop_distance: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AreaInfo<NA> {
    pub area: NA,
    pub network: Graph<(), ()>,
    #[serde(bound(deserialize = "NA: Eq + std::hash::Hash + Deserialize<'de>"))]
    pub nodes: FxHashMap<NA, NodeData>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NodeData {
    pub position: glam::DVec3,
    pub radius: f64,
    pub index: petgraph::graph::NodeIndex,
}

// impl<'de, NA, GA> Deserialize<'de> for AreaInfo<NA, GA>
// where
//     GA: Deserialize<'de>,
//     NA: Deserialize<'de> + Eq + std::hash::Hash,
// {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         #[derive(Deserialize)]
//         #[serde(field_identifier, rename_all = "lowercase")]
//         enum Field {
//             Group,
//             Graph,
//             Nodes,
//         }

//         struct AreaInfoVisitor<NA, GA> {
//             _phantom: PhantomData<fn() -> AreaInfo<NA, GA>>,
//         }

//         impl<'de, NA, GA> serde::de::Visitor<'de> for AreaInfoVisitor<NA, GA> {
//             type Value = AreaInfo<NA, GA>;

//             fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//                 formatter.write_str("struct ")
//             }
//         }

//         todo!()
//     }
// }

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Data<NA, GA, D> {
    pub area: NA,
    pub group: GA,
    pub data: D,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message<NA = Ipv4Addr, GA = Ipv4Addr, D = Box<[u8]>> {
    AreaConstruction(AreaConstruction<NA>),
    JoinReport(JoinReport<NA>),
    #[serde(bound(deserialize = "NA: Eq + std::hash::Hash + Deserialize<'de>"))]
    AreaInfo(AreaInfo<NA>),
    Data(Data<NA, GA, D>),
}

pub type Sender<NA = Ipv4Addr, GA = Ipv4Addr, D = Box<[u8]>> = mpsc::Sender<Message<NA, GA, D>>;
pub type Receiver<NA = Ipv4Addr, GA = Ipv4Addr, D = Box<[u8]>> = mpsc::Receiver<Message<NA, GA, D>>;

macro_rules! into_message_impl {
    ( $t:ty => $v:path ) => {
        impl<NA, GA, D> From<$t> for Message<NA, GA, D> {
            #[inline]
            fn from(value: $t) -> Self {
                $v(value)
            }
        }
    };

    { $( $t:ty => $v:path  ),* $(,)? } => {
        $( into_message_impl!($t => $v); )*
    };
}

into_message_impl! {
    AreaConstruction<NA> => Message::AreaConstruction,
    JoinReport<NA> => Message::JoinReport,
    AreaInfo<NA> => Message::AreaInfo,
    Data<NA, GA, D> => Message::Data,
}

impl<NA, GA, D> Message<NA, GA, D> {
    pub fn area(&self) -> NA
    where
        NA: Copy,
    {
        match self {
            Message::AreaConstruction(m) => m.area,
            Message::JoinReport(m) => m.area,
            Message::AreaInfo(m) => m.area,
            Message::Data(m) => m.area,
        }
    }

    // pub fn is_well_formed(&self) -> bool {
    //     // check is_finite for positions and radii
    //     // check ttl <= k
    // }
}
