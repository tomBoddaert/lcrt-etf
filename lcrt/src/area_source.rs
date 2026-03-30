use std::{net::Ipv4Addr, num::Wrapping};

use petgraph::graph;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{Config, Network, NodeInfo, Response, TimeoutId, availability, message};

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

        let position = node_info.position();

        let mut nodes = FxHashMap::default();
        let mut coverage = petgraph::Graph::new();

        // add the source to the coverage graph and nodes map
        let coverage_index = coverage.add_node(address);
        nodes.insert(
            address,
            ConstructionNode {
                hop_distance: 0,
                position,
                availability: availability(config.bitrate_capacity, node_info.current_bitrate()),
                interfering_neighbours: node_info.interfering_neighbours(),
                coverage_index,
            },
        );

        let m = message::AreaConstruction {
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
                state: State::Construction { nodes, coverage },
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
    pub const fn get_network(&self) -> Option<(&FxHashMap<Ipv4Addr, message::NodeData>, &Network)> {
        let State::Streaming { nodes, network, .. } = &self.state else {
            return None;
        };

        Some((nodes, network))
    }

    #[inline]
    /// If the network is established, returnss the node's children.
    pub const fn get_children(&self) -> Option<&[Ipv4Addr]> {
        let State::Streaming { neighbours, .. } = &self.state else {
            return None;
        };

        Some(neighbours.as_slice())
    }

    #[inline]
    /// Returns whether the network is established and the node has children (and is therefore a forwarder).
    pub const fn has_children(&self) -> bool {
        // Option::map_or is not const, so use match
        match self.get_children() {
            Some(children) => !children.is_empty(),
            None => false,
        }
    }

    /// Returns the next packet ID in the stream.
    pub fn next_packet_id(&mut self) -> Option<u8> {
        let State::Streaming { next_packet_id, .. } = &mut self.state else {
            return None;
        };

        let pid = next_packet_id.0;
        *next_packet_id += 1;
        Some(pid)
    }
}

#[derive(Debug)]
enum State {
    Construction {
        nodes: FxHashMap<Ipv4Addr, ConstructionNode>,
        coverage: petgraph::graph::Graph<Ipv4Addr, ()>,
    },
    Streaming {
        area_info_id: Wrapping<u8>,
        nodes: FxHashMap<Ipv4Addr, message::NodeData>,
        network: graph::Graph<Ipv4Addr, ()>,
        neighbours: Vec<Ipv4Addr>,
        next_packet_id: Wrapping<u8>,
    },
}

#[derive(Debug)]
struct ConstructionNode {
    hop_distance: u16,
    position: glam::DVec3,
    availability: f32,
    interfering_neighbours: u16,
    coverage_index: graph::NodeIndex,
}

impl<N: NodeInfo> AreaSource<N> {
    pub fn handle_timeout(&mut self, id: TimeoutId) -> Response {
        fn extract_level(
            set: &mut FxHashSet<Ipv4Addr>,
            nodes: &FxHashMap<Ipv4Addr, ConstructionNode>,
            level: u16,
        ) {
            set.extend(
                nodes
                    .iter()
                    .filter(|(_, n)| n.hop_distance == level)
                    .map(|(a, _)| *a),
            );
        }

        assert_eq!(id, TimeoutId::Control, "expected a control timeout");

        match &mut self.state {
            State::Construction { nodes, coverage } => {
                // println!("LCRT DEBUG: CONSTRUCTING AREA WITH {} NODES", nodes.len());
                println!("LCRT CONSTRUCTING AREA: \nnodes: {nodes:#?}\ncoverage: {coverage:#?}");
                let mut network = petgraph::Graph::with_capacity(nodes.len(), 0);
                let new_nodes: FxHashMap<Ipv4Addr, message::NodeData> = nodes
                    .iter()
                    .map(|(a, n)| {
                        (
                            *a,
                            message::NodeData {
                                position: n.position,
                                index: network.add_node(*a),
                            },
                        )
                    })
                    .collect();

                let levels = nodes
                    .values()
                    .map(|n| n.hop_distance)
                    .max()
                    .unwrap_or_default();

                // TODO: use rayon?

                let mut l = levels;
                let mut uncovered = FxHashSet::default();
                let mut potential_forwarders = FxHashSet::default();
                let mut neighbours = Vec::new();
                while let Some(new_l) = l.checked_sub(1) {
                    l = new_l;

                    extract_level(&mut uncovered, nodes, l + 1);
                    extract_level(&mut potential_forwarders, nodes, l);

                    while !uncovered.is_empty() {
                        // remove forwarders with no coverage
                        potential_forwarders.retain(|a| {
                            coverage.neighbors(nodes[a].coverage_index).next().is_some()
                        });

                        // find forwarder with highest eta
                        let Some((fa, _)) = potential_forwarders
                            .iter()
                            .copied()
                            .map(|a| {
                                let children = coverage.neighbors(nodes[&a].coverage_index).count();
                                let eta =
                                    nodes[&a].eta(u16::try_from(children).unwrap_or(u16::MAX), 0); // TODO: update interfering nodes
                                (a, eta)
                            })
                            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                        else {
                            // TODO: abandon uncovered nodes?
                            // If nodes are removed from the graph, switch to a StableGraph.
                            // todo!("deal with failed construction");
                            println!("WARNING: ABANDONING NODES {uncovered:?}");
                            uncovered.clear();
                            continue;
                        };
                        // remove the chosen forwarder from the potential forwarders
                        potential_forwarders.remove(&fa);

                        let forwarder_index = new_nodes[&fa].index;
                        let forwarder_coverage_index = nodes[&fa].coverage_index;

                        neighbours.extend(coverage.neighbors(forwarder_coverage_index));

                        for child in neighbours.iter().copied().map(|ni| coverage[ni]) {
                            uncovered.remove(&child);
                            network.add_edge(forwarder_index, new_nodes[&child].index, ());
                        }

                        neighbours.sort_unstable(); // nodes must be removed in reverse-index order
                        for ni in neighbours.iter().copied().rev() {
                            coverage.remove_node(ni);
                        }
                        neighbours.clear();
                    }

                    potential_forwarders.clear();
                }

                let me = new_nodes[&self.address];
                let neighbours: Vec<_> = network.neighbors(me.index).map(|i| network[i]).collect();

                let id = Wrapping(0);
                let m = (!neighbours.is_empty()).then(|| message::AreaInfo {
                    id,
                    network: network.clone(),
                    nodes: new_nodes.clone(),
                });

                self.state = State::Streaming {
                    area_info_id: id,
                    nodes: new_nodes,
                    network,
                    neighbours,
                    next_packet_id: Wrapping(0),
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
            State::Construction { nodes, coverage } => {
                let message::JoinReport {
                    address,
                    hop_distance,
                    position,
                    availability,
                    interfering_neighbours,
                    ..
                } = m;
                println!("{m:?}");

                // deduplicate
                if nodes.contains_key(&address) {
                    return Default::default();
                }

                let coverage_index = coverage.add_node(address);

                let node = ConstructionNode {
                    hop_distance,
                    position,
                    availability,
                    interfering_neighbours,
                    coverage_index,
                };

                let potential_forwarders = nodes
                    .values()
                    .filter(|n| n.hop_distance == hop_distance - 1)
                    .map(|f| (f, &node));
                let potential_children = nodes
                    .values()
                    .filter(|n| n.hop_distance == hop_distance + 1)
                    .map(|c| (&node, c));

                // filter the candidates by coverage and add edges to the graph
                for (f, c) in potential_forwarders
                    .chain(potential_children)
                    .filter(|(f, c)| f.covers(c, self.config.radius))
                {
                    coverage.add_edge(f.coverage_index, c.coverage_index, ());
                }

                nodes.insert(m.address, node);

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
                nodes,
                network,
                neighbours,
                ..
            } => {
                // if we are not the parent and we are not the next forwarder, ignore it
                if m.forwarder == m.address {
                    if m.parent != self.address {
                        return Response::default();
                    }
                    // TODO: temporarily add the node to the neighbours to start forwarding immediately
                    // need to also accept messages on the other end
                } else if !neighbours.contains(&m.forwarder) {
                    return Response::default();
                }

                println!(
                    "{} has received JoinAccept from {}",
                    self.address, m.address
                );

                if let Some(entry) = nodes.remove(&m.address) {
                    // remove subtree rooted at the node
                    let mut to_remove = Vec::new();
                    petgraph::visit::depth_first_search(
                        &*network,
                        std::iter::once(entry.index),
                        |event| match event {
                            petgraph::visit::DfsEvent::Discover(ix, _) => {
                                to_remove.push(ix);
                            }
                            petgraph::visit::DfsEvent::TreeEdge(..)
                            | petgraph::visit::DfsEvent::Finish(..) => {}
                            petgraph::visit::DfsEvent::BackEdge(..)
                            | petgraph::visit::DfsEvent::CrossForwardEdge(..) => {
                                unreachable!("did not expect a non-tree edge")
                            }
                        },
                    );

                    #[cfg(debug_assertions)]
                    {
                        let parent = &nodes[&m.parent];
                        debug_assert!(!to_remove.contains(&parent.index));
                    }

                    // removal must be done in reverse order
                    to_remove.sort_unstable();
                    for ix in to_remove.into_iter().rev() {
                        let last_ix = network
                            .node_indices()
                            .next_back()
                            .expect("expected network to contain at least one node");

                        // set the last node's index to the removed index
                        if ix != last_ix {
                            let last_node = nodes
                                .get_mut(&network[last_ix])
                                .expect("expected node from network to exist in nodes map");
                            last_node.index = ix;
                        }

                        // remove the node
                        let id = network
                            .remove_node(ix)
                            .expect("expected node from network to exist in the network");
                        println!("Removing node {id}");
                        let removed = nodes.remove(&id);
                        if id == m.address {
                            debug_assert!(removed.is_none());
                            debug_assert_eq!(ix, entry.index);
                        } else {
                            debug_assert_eq!(removed.map(|node| node.index), Some(ix));
                        }
                    }
                }

                let ix = network.add_node(m.address);
                nodes.insert(
                    m.address,
                    message::NodeData {
                        position: m.position,
                        index: ix,
                    },
                );
                let parent = nodes[&m.parent];
                network.add_edge(parent.index, ix, ());

                println!("{network:?}");

                let me = nodes[&self.address];
                neighbours.clear();
                neighbours.extend(network.neighbors(me.index).map(|i| network[i]));

                *area_info_id += 1;
                (!neighbours.is_empty())
                    .then(|| message::AreaInfo {
                        id: *area_info_id,
                        network: network.clone(),
                        nodes: nodes.clone(),
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
            self.interfering_neighbours + added_interfering_nodes,
        )
    }

    fn covers(&self, other: &Self, radius: f64) -> bool {
        self.position.distance_squared(other.position) <= radius * radius
    }
}
