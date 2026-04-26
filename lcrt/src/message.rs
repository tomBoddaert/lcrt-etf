//! LCRT area control message definitions.

use std::{
    net::Ipv4Addr,
    num::{NonZero, Wrapping},
};

use petgraph::stable_graph;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
/// The message that advertises the construction of a new LCRT area.
pub struct AreaConstruction {
    /// Time To Live (TTL). Must be decremented each time the message is forwarded.
    pub ttl: NonZero<u16>,
    /// Position of the forwarding node.
    pub position: glam::DVec3,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
/// The message requesting to join an LCRT area.
pub struct JoinReport {
    /// Address of the joining node.
    pub address: Ipv4Addr,
    /// Hop distance from the source to the joining node.
    pub hop_distance: u16,
    /// Position of the joining node.
    pub position: glam::DVec3,
    /// Avaliability of the joining node.
    pub availability: f32,
    /// Number of transmitting neighbours in interference range of the joining node.
    pub interfering_neighbours: u16,
    /// Hop distance from the source to the forwarding node.
    pub forwarder_hop_distance: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// The message signalling the creation of an LCRT area.
pub struct AreaInfo {
    /// Id for this area info.
    pub id: Wrapping<u8>,
    /// Network routing graph.
    pub network: stable_graph::StableGraph<Ipv4Addr, ()>, // TODO: switch back to regular graph / CSR
    /// [`NodeData`] map.
    pub nodes: FxHashMap<Ipv4Addr, NodeData>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
/// Information about a node in an LCRT area network.
pub struct NodeData {
    /// The node's position.
    pub position: glam::DVec3,
    /// The node's graph index in the network routing graph (from [`AreaInfo::network`]).
    pub index: stable_graph::NodeIndex,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct JoinArea {
    pub address: Ipv4Addr,
    pub position: glam::DVec3,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct JoinAvailable {
    pub address: Ipv4Addr,
    pub parent: Ipv4Addr,
    pub hop_distance: u16,
    pub confidence: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct JoinAccept {
    pub address: Ipv4Addr,
    pub position: glam::DVec3,
    pub parent: Ipv4Addr,
    pub forwarder: Ipv4Addr,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// An LCRT area control message.
pub enum Message {
    AreaConstruction(AreaConstruction),
    JoinReport(JoinReport),
    AreaInfo(AreaInfo),
    JoinArea(JoinArea),
    JoinAvailable(JoinAvailable),
    JoinAccept(JoinAccept),
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
    JoinArea => Message::JoinArea,
    JoinAvailable => Message::JoinAvailable,
    JoinAccept => Message::JoinAccept,
}
