//! An implementation of the Link-Controlled Routing Tree algorithm.
//!
//! Based on the paper "[Resource-Aware Video Multicasting via Access Gateways in Wireless Mesh Networks](https://www.doi.org/10.1109/ICNP.2008.4697023)" by W. Tu, C. J. Sreenan, C. T. Chou, A. Misra and S. Jha, published in 2008 IEEE International Conference on Network Protocols, pp. 43-52. doi: [10.1109/ICNP.2008.4697023](https://www.doi.org/10.1109/ICNP.2008.4697023).

use std::net::Ipv4Addr;

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
mod config;
pub mod message;
mod node_info;
mod response;

pub use area::Area;
pub use area_any::AreaAny;
pub use area_source::AreaSource;
pub use config::Config;
pub use node_info::NodeInfo;
pub use response::{Event, Response, Timeout, TimeoutId};

/// A graph representing an LCRT area network.
pub type Network = stable_graph::StableGraph<Ipv4Addr, ()>; // TODO: switch to regular graph / CSR

fn availability(capacity: f32, rate: f32) -> f32 {
    capacity / rate
}

fn eta(availability: f32, children: u16, interfering_nodes: u16) -> f32 {
    f32::from(children) / f32::from(1 + interfering_nodes) * availability
}
