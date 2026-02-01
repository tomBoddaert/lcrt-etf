mod area;
mod area_any;
mod area_source;
mod config;
pub mod message;
mod node_info;
use std::{net::Ipv4Addr, time};

pub use area::Area;
pub use area_any::AreaAny;
pub use area_source::AreaSource;
pub use config::Config;
pub use node_info::NodeInfo;
use petgraph::graph;

pub type Network = graph::Graph<Ipv4Addr, ()>;
pub type Response = (Option<message::Message>, Option<time::Duration>);

fn availability(capacity: f32, rate: f32) -> f32 {
    capacity / rate
}

fn eta(availability: f32, children: u16, interfering_nodes: u16) -> f32 {
    f32::from(children) / f32::from(1 + interfering_nodes) * availability
}
