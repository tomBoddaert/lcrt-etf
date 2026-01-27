use std::{net::Ipv4Addr, num::NonZero};

use petgraph::graph;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AreaConstruction {
    pub ttl: NonZero<u16>,
    pub position: glam::DVec3,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct JoinReport {
    pub address: Ipv4Addr,
    pub hop_distance: u16,
    pub position: glam::DVec3,
    pub availability: f32,
    pub interfering_neighbours: u16,
    pub forwarder_hop_distance: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AreaInfo {
    pub network: graph::Graph<Ipv4Addr, ()>,
    pub nodes: FxHashMap<Ipv4Addr, NodeData>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NodeData {
    pub position: glam::DVec3,
    pub index: graph::NodeIndex,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    AreaConstruction(AreaConstruction),
    JoinReport(JoinReport),
    AreaInfo(AreaInfo),
}

macro_rules! into_message_impl {
    ( $t:ty => $v:path ) => {
        impl From<$t> for Message {
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
    AreaConstruction => Message::AreaConstruction,
    JoinReport => Message::JoinReport,
    AreaInfo => Message::AreaInfo,
}
