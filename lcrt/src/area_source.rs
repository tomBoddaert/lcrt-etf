use std::{net::Ipv4Addr, time};

use petgraph::graph;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{Config, Network, NodeInfo, availability, message};

pub struct AreaSource<N> {
    config: Config,
    address: Ipv4Addr,
    group: Ipv4Addr,
    node_info: N,
    state: State,
}

impl<N: NodeInfo> AreaSource<N> {
    pub fn new(
        config: Config,
        node_info: N,
        address: Ipv4Addr,
        group: Ipv4Addr,
    ) -> (Self, Option<message::Message>, Option<time::Duration>) {
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
        }
        .into();
        (
            Self {
                config,
                address,
                group,
                node_info,
                state: State::Construction { nodes, coverage },
            },
            Some(m),
            Some(config.source_construct_timeout),
        )
    }

    #[inline]
    pub const fn get_address(&self) -> Ipv4Addr {
        self.address
    }

    #[inline]
    pub const fn get_group(&self) -> Ipv4Addr {
        self.group
    }

    pub const fn is_streaming(&self) -> bool {
        matches!(&self.state, State::Streaming { .. })
    }

    pub const fn get_network(&self) -> Option<(&FxHashMap<Ipv4Addr, message::NodeData>, &Network)> {
        let State::Streaming { nodes, network, .. } = &self.state else {
            return None;
        };

        Some((nodes, network))
    }

    pub fn is_forwarder(&self, dst: Ipv4Addr) -> bool {
        // println!(
        //     "AreaSource::is_forwarder(self.group: {}, dst: {}) -> {}",
        //     self.group,
        //     dst,
        //     self.group == dst
        // );
        if dst != self.group {
            return false;
        }

        let State::Streaming { neighbours, .. } = &self.state else {
            // TODO: make an error
            return false;
        };

        !neighbours.is_empty()
    }

    pub fn get_next_hops(&self, dst: Ipv4Addr) -> (&[Ipv4Addr], bool) {
        if dst != self.group {
            return (&[], false);
        }

        let State::Streaming { neighbours, .. } = &self.state else {
            // TODO: make an error / possibly true 'adressed to us' (second part of tuple)?
            return (&[], false);
        };

        (neighbours, true)
    }

    pub const fn get_hop_distance(&self) -> Option<u16> {
        let State::Streaming { .. } = &self.state else {
            return None;
        };

        Some(0)
    }
}

#[derive(Debug)]
enum State {
    Construction {
        nodes: FxHashMap<Ipv4Addr, ConstructionNode>,
        coverage: petgraph::graph::Graph<Ipv4Addr, ()>,
    },
    Streaming {
        nodes: FxHashMap<Ipv4Addr, message::NodeData>,
        network: graph::Graph<Ipv4Addr, ()>,
        neighbours: Vec<Ipv4Addr>,
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
    pub fn handle_timeout(&mut self) -> (Option<message::Message>, Option<time::Duration>) {
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

        match &mut self.state {
            State::Construction { nodes, coverage } => {
                // println!("LCRT DEBUG: CONSTRUCTING AREA WITH {} NODES", nodes.len());
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

                let m = message::AreaInfo {
                    network: network.clone(),
                    nodes: new_nodes.clone(),
                }
                .into();

                self.state = State::Streaming {
                    nodes: new_nodes,
                    network,
                    neighbours,
                };

                (Some(m), None)
            }

            _ => todo!(),
        }
    }

    pub fn handle_message(
        &mut self,
        m: message::Message,
    ) -> (Option<message::Message>, Option<time::Duration>) {
        match m {
            message::Message::AreaConstruction(_) | message::Message::AreaInfo(_) => {
                // TODO: verify consistency?
                Default::default()
            }

            message::Message::JoinReport(join_report) => self.handle_join_report(join_report),
        }
    }

    pub fn handle_join_report(
        &mut self,
        m: message::JoinReport,
    ) -> (Option<message::Message>, Option<time::Duration>) {
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

                (None, Some(self.config.source_construct_timeout))
            }

            State::Streaming { .. } => {
                // too late
                // TODO: emit warning?
                Default::default()
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
