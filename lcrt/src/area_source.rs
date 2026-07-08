use std::{net::Ipv4Addr, num::Wrapping};

use common::geo::Sphere;
use petgraph::stable_graph;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    Config, Network, NodeInfo, Response, TimeoutId, behaviour::Sending,
    construction::SystemConstruction, message,
};

/// Routing controller for an LCRT area source.
pub struct AreaSource<N> {
    config: Config,
    address: Ipv4Addr,
    group: Ipv4Addr,
    node_info: N,
    state: State,
}

impl<N: NodeInfo> AreaSource<N> {
    /// Construct a new source area routing controller.
    ///
    /// # Panics
    /// This will panic if `config` is not valid (see [`Config::is_valid`]).
    pub fn new(
        config: Config,
        node_info: N,
        address: Ipv4Addr,
        group: Ipv4Addr,
    ) -> (Self, Response) {
        assert!(config.is_valid());

        let position = Sphere::new(node_info.position(), config.radius);

        let mut construction = SystemConstruction::new();
        construction.add(crate::construction::ConstructionNode {
            node: message::NodeData { address, position },
            hop_distance: 0,
            // this is the only node in level 0, so the metrics do not matter
            availability: f32::INFINITY,
            interfering_neighbours: 0,
        });

        let m = message::AreaConstruction {
            k: config.k,
            ttl: config.k,
            position,
        };
        let t = (TimeoutId::Control, config.source_construct_timeout);
        (
            Self {
                config,
                address,
                group,
                node_info,
                state: State::Construction {
                    addresses: FxHashSet::default(),
                    construction,
                },
            },
            (m, t).into(),
        )
    }

    #[inline]
    /// Get the node's address.
    pub const fn get_address(&self) -> Ipv4Addr {
        self.address
    }

    #[inline]
    /// Get the group address for the area.
    pub const fn get_group(&self) -> Ipv4Addr {
        self.group
    }

    #[inline]
    pub const fn get_config(&self) -> &Config {
        &self.config
    }

    #[inline]
    pub const fn get_node_info(&self) -> &N {
        &self.node_info
    }

    #[inline]
    /// Returns whether this routing controller has established an area and is able to send data streams.
    pub const fn is_streaming(&self) -> bool {
        matches!(&self.state, State::Streaming { .. })
    }

    #[inline]
    /// If the network is established, returns the network topology graph and [`message::NodeData`] map.
    pub const fn get_network(&self) -> Option<&Network> {
        let State::Streaming { network, .. } = &self.state else {
            return None;
        };

        Some(network)
    }

    #[inline]
    /// If the network is established, returnss the node's children.
    pub const fn get_children(&self) -> Option<&[Ipv4Addr]> {
        let State::Streaming { sending, .. } = &self.state else {
            return None;
        };

        Some(sending.get_children())
    }

    #[inline]
    /// Returns whether the network is established and the node has children (and is therefore a forwarder).
    pub const fn has_children(&self) -> bool {
        let State::Streaming { sending, .. } = &self.state else {
            return false;
        };

        sending.has_children()
    }

    #[inline]
    /// Returns the next packet ID in the stream.
    pub fn next_packet_id(&mut self) -> Option<u8> {
        let State::Streaming { sending, .. } = &mut self.state else {
            return None;
        };

        Some(sending.next_packet_id().0)
    }
}

// TODO: enable debug once Graph has debug impl
// #[derive(Debug)]
enum State {
    Construction {
        addresses: FxHashSet<Ipv4Addr>,
        construction: SystemConstruction,
    },
    Streaming {
        area_info_id: Wrapping<u8>,
        network: Network,
        sending: Sending,
    },
}

#[derive(Debug)]
struct ConstructionNode {
    hop_distance: u16,
    position: Sphere,
    availability: f32,
    interfering_neighbours: u16,
    coverage_index: stable_graph::NodeIndex,
}

impl<N: NodeInfo> AreaSource<N> {
    pub fn handle_timeout(&mut self, id: TimeoutId) -> Response {
        assert_eq!(id, TimeoutId::Control, "expected a control timeout");

        match &mut self.state {
            State::Construction { construction, .. } => {
                let construction = std::mem::replace(construction, SystemConstruction::new());
                let network = construction.construct();
                // let (nodes, network) = network.take_nodes();

                let sending = Sending::new(&network, self.address)
                    .expect("[unreachable] expected this node (the root) to be in the network");

                let id = Wrapping(0);
                let m = sending.has_children().then(|| message::AreaInfo {
                    id,
                    network: network.clone(),
                    next_packet_id: sending.peek_next_packet_id(),
                });
                // println!("{m:?}");

                self.state = State::Streaming {
                    area_info_id: id,
                    network,
                    sending,
                };

                m.into()
            }

            _ => todo!(),
        }
    }

    pub fn handle_message(&mut self, m: message::Message) -> Response {
        match m {
            message::Message::AreaConstruction(_) | message::Message::AreaInfo(_) => {
                // TODO: verify consistency?
                Default::default()
            }

            message::Message::JoinReport(join_report) => self.handle_join_report(join_report),

            message::Message::JoinArea(join_group) => self.handle_join_area(join_group),
            message::Message::JoinAvailable(_) => Default::default(),
            message::Message::JoinAccept(join_accept) => self.handle_join_accept(join_accept),
        }
    }

    pub fn handle_join_report(&mut self, m: message::JoinReport) -> Response {
        match &mut self.state {
            State::Construction {
                addresses,
                construction,
            } => {
                let message::JoinReport {
                    address,
                    hop_distance,
                    position,
                    availability,
                    interfering_neighbours,
                    ..
                } = m;

                // TODO: move deduplication logic to construction system?
                // deduplicate
                if addresses.contains(&address) {
                    return Default::default();
                }

                construction.add(crate::construction::ConstructionNode {
                    node: message::NodeData { address, position },
                    hop_distance,
                    availability,
                    interfering_neighbours,
                });

                addresses.insert(m.address);

                (TimeoutId::Control, self.config.source_construct_timeout).into()
            }

            State::Streaming { .. } => {
                // too late
                // TODO: emit warning?
                Default::default()
            }
        }
    }

    pub fn handle_join_area(&mut self, m: message::JoinArea) -> Response {
        // are we within RTR?
        let position = self.node_info.position();
        if position.distance_squared(m.position) > self.config.radius * self.config.radius {
            return Default::default();
        }

        message::JoinAvailable {
            address: m.address,
            parent: self.address,
            hop_distance: 1,
            confidence: 1.,
        }
        .into()
    }

    pub fn handle_join_accept(&mut self, m: message::JoinAccept) -> Response {
        match &mut self.state {
            State::Construction { .. } => todo!(),

            State::Streaming {
                area_info_id,
                network,
                sending,
            } => {
                // if we are not the parent and we are not the next forwarder, ignore it
                if m.forwarder == m.address {
                    if m.parent != self.address {
                        return Response::default();
                    }
                    // TODO: temporarily add the node to the neighbours to start forwarding immediately
                    // need to also accept messages on the other end
                } else if !sending.get_children().contains(&m.forwarder) {
                    return Response::default();
                }

                // println!(
                //     "{} has received JoinAccept from {}",
                //     self.address, m.address
                // );

                if sending.get_children().contains(&m.address) {
                    todo!("handle [re]moving subtree rooted at joining node");
                }

                let n = network.add_node(message::NodeData {
                    address: m.address,
                    position: m.position,
                });
                let p = network
                    .nodes()
                    .iter()
                    .position(|node| node.address == m.parent)
                    .expect("expected parent to be in network");
                network.add_edge(p, n, message::Edge::Link);

                network.edit_edge_pairs_for_node(
                    |a, b, ab, ba| {
                        let Some((abi, bai)) = a.position.intersections(&b.position) else {
                            debug_assert!(ab.is_none() && ba.is_none());
                            return (None, None);
                        };

                        let e = if abi && bai {
                            message::Edge::Connection
                        } else {
                            message::Edge::Intersection
                        };

                        // don't override the parent -link> node edge
                        (Some(e), ba.or(Some(e)))
                    },
                    n,
                );

                *sending = std::mem::replace(sending, Sending::dummy())
                    .update(&network, self.address)
                    .expect("[unreachable] expected this node (the root) to be in the network");

                // println!("{network:?}");

                *area_info_id += 1;
                sending
                    .has_children()
                    .then(|| message::AreaInfo {
                        id: *area_info_id,
                        network: network.clone(),
                        next_packet_id: sending.peek_next_packet_id(),
                    })
                    .into()
            }
        }
    }
}

impl ConstructionNode {
    fn eta(&self, children: u16, added_interfering_nodes: u16) -> f32 {
        crate::eta(
            self.availability,
            children,
            f32::from(self.interfering_neighbours) + f32::from(added_interfering_nodes),
        )
    }
}

fn delete_tree(
    graph: &mut stable_graph::StableGraph<Ipv4Addr, ()>,
    nodes: &mut FxHashMap<Ipv4Addr, message::NodeData>,
    root: stable_graph::NodeIndex,
) {
    let id = *graph
        .node_weight(root)
        .expect("expected node to remove to exist in network");
    print!("{}, ", id);

    let mut neighbours = graph.neighbors(root).detach();
    while let Some(neighbour) = neighbours.next_node(graph) {
        delete_tree(graph, nodes, neighbour);
    }

    nodes.remove(&id);
    graph.remove_node(root);
}
