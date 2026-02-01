use std::{mem, net::Ipv4Addr, num::NonZero, time};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{Config, Network, NodeInfo, availability, message};

pub struct Area<N> {
    config: Config,
    address: Ipv4Addr,
    group: Ipv4Addr,
    node_info: N,
    state: State,
}

impl<N: NodeInfo> Area<N> {
    pub const fn new(config: Config, node_info: N, address: Ipv4Addr, group: Ipv4Addr) -> Self {
        assert!(config.is_valid());

        Self {
            config,
            address,
            group,
            node_info,
            state: State::Startup,
        }
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
        //     "Area::is_forwarder(self.group: {}, dst: {}) -> {}",
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

    pub fn is_parent(&self, last_forwarder: Ipv4Addr) -> bool {
        let State::Streaming { parent, .. } = &self.state else {
            // println!(
            //     "is_parent(self: {}, last_forwarder: {}) -> false (NOT STREAMING)",
            //     self.address, last_forwarder
            // );
            // TODO: make an error
            return false;
        };

        // println!(
        //     "is_parent(self: {}, last_forwarder: {}) -> {}",
        //     self.address,
        //     last_forwarder,
        //     last_forwarder == *parent
        // );

        last_forwarder == *parent
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
        let State::Streaming { hop_distance, .. } = &self.state else {
            return None;
        };

        Some(*hop_distance)
    }
}

enum State {
    Startup,
    Construction {
        min_hop_distance: u16,
        position: glam::DVec3,
        joins_forwarded: FxHashSet<Ipv4Addr>,
    },
    AwaitingAreaInfo {
        hop_distance: u16,
        joins_forwarded: FxHashSet<Ipv4Addr>,
    },
    Streaming {
        hop_distance: u16,
        nodes: FxHashMap<Ipv4Addr, message::NodeData>,
        network: Network,
        neighbours: Vec<Ipv4Addr>,
        parent: Ipv4Addr,
    },
}

impl<N: NodeInfo> Area<N> {
    pub fn handle_timeout(&mut self) -> (Option<message::Message>, Option<time::Duration>) {
        match &mut self.state {
            State::Construction {
                min_hop_distance,
                position,
                joins_forwarded,
            } => {
                let m = message::JoinReport {
                    address: self.address,
                    hop_distance: *min_hop_distance,
                    position: *position,
                    availability: availability(
                        self.config.bitrate_capacity,
                        self.node_info.current_bitrate(),
                    ),
                    interfering_neighbours: self.node_info.interfering_neighbours(),
                    forwarder_hop_distance: *min_hop_distance,
                }
                .into();

                self.state = State::AwaitingAreaInfo {
                    hop_distance: *min_hop_distance,
                    joins_forwarded: mem::take(joins_forwarded),
                };

                (Some(m), None)
            }

            _ => todo!("error? maybe only in debug"),
        }
    }

    pub fn handle_message(
        &mut self,
        m: message::Message,
    ) -> (Option<message::Message>, Option<time::Duration>) {
        match m {
            message::Message::AreaConstruction(area_construction) => {
                self.handle_area_construction(area_construction)
            }
            message::Message::JoinReport(join_report) => self.handle_join_report(join_report),
            message::Message::AreaInfo(area_info) => self.handle_area_info(area_info),
        }
    }

    pub fn handle_area_construction(
        &mut self,
        m: message::AreaConstruction,
    ) -> (Option<message::Message>, Option<time::Duration>) {
        match &mut self.state {
            State::Startup => {
                let position = self.node_info.position();

                // if either node is outside of the other's RTR, ignore it
                if position.distance_squared(m.position) > self.config.radius * self.config.radius {
                    return Default::default();
                }

                let ttl = m.ttl.get() - 1;
                debug_assert_ne!(self.config.k.get(), ttl); // TODO: use NonZero for hop_distance?

                self.state = State::Construction {
                    min_hop_distance: self.config.k.get() - ttl,
                    position,
                    joins_forwarded: FxHashSet::default(),
                };

                let m =
                    NonZero::new(ttl).map(|ttl| message::AreaConstruction { ttl, position }.into());
                (m, Some(self.config.construct_timeout))
            }

            State::Construction {
                min_hop_distance,
                position,
                ..
            } => {
                // if either node is outside of the other's RTR, ignore it
                if position.distance_squared(m.position) > self.config.radius * self.config.radius {
                    return Default::default();
                }

                let ttl = m.ttl.get() - 1;
                debug_assert!(self.config.k.get() > ttl);
                let hop_distance = self.config.k.get() - ttl;

                // if the hop distance is no better, ignore it
                if hop_distance >= *min_hop_distance {
                    return Default::default();
                }

                // TODO: handle error
                // assuming k has stayed constant, hd < mhd, so ttl > maxttl >= 0
                // if this fails, then k must have changed
                let ttl = NonZero::new(ttl).expect("expected improved ttl to be nonzero");
                *min_hop_distance = hop_distance;

                let m = message::AreaConstruction {
                    ttl,
                    position: *position,
                }
                .into();
                (Some(m), Some(self.config.construct_timeout))
            }

            _ => {
                // TODO: log error / warning?
                Default::default()
            }
        }
    }

    pub fn handle_join_report(
        &mut self,
        mut m: message::JoinReport,
    ) -> (Option<message::Message>, Option<time::Duration>) {
        match &mut self.state {
            State::Startup => {
                // TODO cache join requests to be sent later
                Default::default()
            }

            State::Construction {
                min_hop_distance: hop_distance,
                joins_forwarded,
                ..
            }
            | State::AwaitingAreaInfo {
                hop_distance,
                joins_forwarded,
            } => {
                if *hop_distance >= m.forwarder_hop_distance || joins_forwarded.contains(&m.address)
                {
                    return Default::default();
                }

                m.forwarder_hop_distance = *hop_distance;

                joins_forwarded.insert(m.address);

                (Some(m.into()), None)
            }

            State::Streaming { .. } => {
                // TODO: too late, log an error / warning
                Default::default()
            }
        }
    }

    pub fn handle_area_info(
        &mut self,
        m: message::AreaInfo,
    ) -> (Option<message::Message>, Option<time::Duration>) {
        match &mut self.state {
            State::Startup | State::Construction { .. } => {
                println!("WARNING: NODE {} NOT IN AREA {}", self.address, self.group);
                Default::default()
            }

            State::AwaitingAreaInfo { hop_distance, .. } => {
                let message::AreaInfo { network, nodes } = m;

                let Some(me) = nodes.get(&self.address) else {
                    println!("WARNING: {} NOT INCLUDED IN AREA", self.address);
                    self.state = State::Startup;
                    return Default::default();
                };
                let neighbours: Vec<_> = network.neighbors(me.index).map(|i| network[i]).collect();
                let mut parents = network.neighbors_directed(me.index, petgraph::Incoming);
                let parent = parents
                    .next()
                    .map(|i| network[i])
                    .expect("expected to have a parent in the network");
                debug_assert!(
                    parents.next().is_none(),
                    "expected to have no more than one parent in the network"
                );

                let m = (!neighbours.is_empty()).then_some(
                    message::AreaInfo {
                        network: network.clone(),
                        nodes: nodes.clone(),
                    }
                    .into(),
                );

                self.state = State::Streaming {
                    hop_distance: *hop_distance,
                    nodes,
                    network,
                    neighbours,
                    parent,
                };

                (m, None)
            }

            State::Streaming { .. } => {
                // TODO: any reason not to ignore it here?
                // most likely a repeat but could be used for updates
                Default::default()
            }
        }
    }
}
