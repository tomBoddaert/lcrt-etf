//! LCRT area control message definitions.

use std::{
    hash::Hash,
    net::Ipv4Addr,
    num::{NonZero, Wrapping},
};

use common::geo::Sphere;
use petgraph::stable_graph;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::Identifier;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
/// The message that advertises the construction of a new LCRT area.
pub struct AreaConstruction {
    pub k: NonZero<u16>,
    /// Time To Live (TTL). Must be decremented each time the message is forwarded.
    pub ttl: NonZero<u16>,
    /// Position and radius of the forwarding node.
    pub position: Sphere,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
/// The message requesting to join an LCRT area.
pub struct JoinReport<Id = Ipv4Addr> {
    /// Address of the joining node.
    pub address: Id,
    /// Hop distance from the source to the joining node.
    pub hop_distance: u16,
    /// Position of the joining node.
    pub position: glam::DVec3,
    /// Availability of the joining node.
    pub availability: f32,
    /// Number of transmitting neighbours in interference range of the joining node.
    pub interfering_neighbours: u16,
    /// Hop distance from the source to the forwarding node.
    pub forwarder_hop_distance: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// The message signalling the creation of an LCRT area.
pub struct AreaInfo<Id = Ipv4Addr>
where
    Id: Eq + Hash,
{
    /// Id for this area info.
    pub id: Wrapping<u8>,
    /// Network routing graph.
    pub network: stable_graph::StableGraph<Id, ()>, // TODO: switch back to regular graph / CSR
    /// [`NodeData`] map.
    pub nodes: FxHashMap<Id, NodeData>,
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
pub struct JoinArea<Id = Ipv4Addr> {
    pub address: Id,
    pub position: glam::DVec3,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct JoinAvailable<Id = Ipv4Addr> {
    pub address: Id,
    pub parent: Id,
    pub hop_distance: u16,
    pub confidence: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct JoinAccept<Id = Ipv4Addr> {
    pub address: Id,
    pub position: glam::DVec3,
    pub parent: Id,
    pub forwarder: Id,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// An LCRT area control message.
pub enum Message<Id = Ipv4Addr>
where
    Id: Eq + Hash,
{
    AreaConstruction(AreaConstruction),
    JoinReport(JoinReport<Id>),
    AreaInfo(AreaInfo<Id>),
    JoinArea(JoinArea<Id>),
    JoinAvailable(JoinAvailable<Id>),
    JoinAccept(JoinAccept<Id>),
}

macro_rules! into_message_impl {
    ( < $id:ident > $t:ty => $v:path ) => {
        impl<$id> From<$t> for Message<$id>
        where
            $id: Identifier,
        {
            #[inline]
            fn from(value: $t) -> Self {
                $v(value)
            }
        }
    };

    { < $id:ident > $( $t:ty => $v:path  ),* $(,)? } => {
        $( into_message_impl!(<$id> $t => $v); )*
    };
}

into_message_impl! { <Id>
    AreaConstruction => Message::AreaConstruction,
    JoinReport<Id> => Message::JoinReport,
    AreaInfo<Id> => Message::AreaInfo,
    JoinArea<Id> => Message::JoinArea,
    JoinAvailable<Id> => Message::JoinAvailable,
    JoinAccept<Id> => Message::JoinAccept,
}
