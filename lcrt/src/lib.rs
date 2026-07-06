//! An implementation of the Link-Controlled Routing Tree algorithm.
//!
//! Based on the paper "[Resource-Aware Video Multicasting via Access Gateways in Wireless Mesh Networks](https://www.doi.org/10.1109/ICNP.2008.4697023)" by W. Tu, C. J. Sreenan, C. T. Chou, A. Misra and S. Jha, published in 2008 IEEE International Conference on Network Protocols, pp. 43-52. doi: [10.1109/ICNP.2008.4697023](https://www.doi.org/10.1109/ICNP.2008.4697023).

use std::{hash::Hash, net::Ipv4Addr};

use petgraph::stable_graph;

macro_rules! doc_handle_return {
    () => {
        concat!(
            "Possibly returns:\n",
            "- A message to broadcast to neighbours.\n",
            "- A duration to wait before calling [`Self::handle_timeout`]. This **must override** any timers previously set by this area controller."
        )
    };
}

mod area;
mod area_any;
mod area_source;
pub mod behaviour;
mod config;
mod construction;
pub mod message;
mod node_info;
mod response;
pub mod trace;

pub use area::Area;
pub use area_any::AreaAny;
pub use area_source::AreaSource;
pub use config::Config;
pub use node_info::NodeInfo;
pub use response::{Event, Response, Timeout, TimeoutId};

/// A graph representing an LCRT area network.
pub type Network = stable_graph::StableGraph<Ipv4Addr, ()>; // TODO: switch to regular graph / CSR

/// Alias for [`Copy`]` + `[`Eq`]` + `[`Hash`].
///
/// Usually defaults to [`Ipv4Addr`].
pub trait Identifier: Copy + Eq + Hash {}
impl<Id> Identifier for Id where Id: Copy + Eq + Hash {}

fn availability(capacity: f32, rate: f32) -> f32 {
    capacity / rate
}

fn eta(availability: f32, children: u16, interfering_nodes: f32) -> f32 {
    // add 1 to interfering nodes to avoid divide by zero
    f32::from(children) / (1. + interfering_nodes) * availability
}

#[cfg(test)]
mod test {
    pub mod tree_example {
        use common::geo::Sphere;

        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct NodeInfo {
            pub id: &'static str,
            pub sphere: Sphere,
            pub hop_distance: u16,
            pub potential_children: &'static [&'static str],
            pub children: &'static [&'static str],
        }

        pub const NODES: &[NodeInfo] = &[
            NodeInfo {
                id: "A",
                sphere: Sphere::with_components(70., 140., 35., 50.),
                hop_distance: 0,
                potential_children: &["B", "C"],
                children: &["B", "C"],
            },
            NodeInfo {
                id: "B",
                sphere: Sphere::with_components(40., 110., 20., 50.),
                hop_distance: 1,
                potential_children: &["D", "E"],
                children: &["D", "E"],
            },
            NodeInfo {
                id: "C",
                sphere: Sphere::with_components(110., 120., 20., 50.),
                hop_distance: 1,
                potential_children: &["F"],
                children: &["F"],
            },
            NodeInfo {
                id: "D",
                sphere: Sphere::with_components(0., 110., 0., 50.),
                hop_distance: 2,
                potential_children: &[],
                children: &[],
            },
            NodeInfo {
                id: "E",
                sphere: Sphere::with_components(20., 70., 20., 50.),
                hop_distance: 2,
                potential_children: &["G"],
                children: &["G"],
            },
            NodeInfo {
                id: "F",
                sphere: Sphere::with_components(120., 80., 40., 50.),
                hop_distance: 2,
                potential_children: &["H"],
                children: &["H"],
            },
            NodeInfo {
                id: "G",
                sphere: Sphere::with_components(10., 30., 30., 50.),
                hop_distance: 3,
                potential_children: &["K"],
                children: &["K"],
            },
            NodeInfo {
                id: "H",
                sphere: Sphere::with_components(120., 40., 20., 50.),
                hop_distance: 3,
                potential_children: &["I"],
                children: &["I"],
            },
            NodeInfo {
                id: "I",
                sphere: Sphere::with_components(100., 0., 30., 50.),
                hop_distance: 4,
                potential_children: &[],
                children: &[],
            },
            NodeInfo {
                id: "K",
                sphere: Sphere::with_components(0., 20., 50., 50.),
                hop_distance: 4,
                potential_children: &[],
                children: &[],
            },
        ];

        pub const MAX_HOP_DISTANCE: u16 = {
            let mut max = 0;
            let mut i = 0;
            while i < NODES.len() {
                let hd = NODES[i].hop_distance;
                if hd > max {
                    max = hd;
                }
                i += 1;
            }
            max
        };
        pub const FORWARDERS: &[&str] = &{
            const LEN: usize = {
                let mut i = 0;
                let mut j = 0;
                while i < NODES.len() {
                    let node = &NODES[i];
                    if !node.children.is_empty() {
                        j += 1;
                    }
                    i += 1;
                }
                j
            };

            let mut buffer = [""; LEN];

            let mut i = 0;
            let mut j = 0;
            while i < NODES.len() {
                let node = &NODES[i];
                if !node.children.is_empty() {
                    buffer[j] = node.id;
                    j += 1;
                }
                i += 1;
            }

            buffer
        };
    }
}
