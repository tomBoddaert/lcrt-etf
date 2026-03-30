use std::{
    mem,
    net::Ipv4Addr,
    num::{NonZero, Wrapping},
};

use common::AncestorWalker;
use petgraph::visit::Walker;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    Config, Event, Network, NodeInfo, Response, Timeout, TimeoutId, availability, message,
};

/// Routing controller for an LCRT area non-source member (forwarder / receiver).
pub struct Area<N> {
    config: Config,
    address: Ipv4Addr,
    group: Ipv4Addr,
    node_info: N,
    state: State,
}

impl<N: NodeInfo> Area<N> {
    /// Construct a new non-source area routing controller.
    ///
    /// # Panics
    /// This will panic if `config` is not valid (see [`Config::is_valid`]).
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
    /// Returns whether this routing controller has established an area and is able to receive data streams.
    pub const fn is_streaming(&self) -> bool {
        matches!(&self.state, State::Streaming { .. })
    }

    #[inline]
    /// If the network is established, returns the network topology graph and [`NodeData`](message::NodeData) map.
    pub const fn get_network(&self) -> Option<(&FxHashMap<Ipv4Addr, message::NodeData>, &Network)> {
        let State::Streaming { nodes, network, .. } = &self.state else {
            return None;
        };

        Some((nodes, network))
    }

    #[inline]
    /// If the network is established, returns the node's parent.
    pub const fn get_parent(&self) -> Option<Ipv4Addr> {
        let State::Streaming { parent, .. } = &self.state else {
            return None;
        };

        Some(*parent)
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

    #[inline]
    /// If the network is established, returns the node's hop distance from the area source.
    pub const fn get_hop_distance(&self) -> Option<u16> {
        let State::Streaming { hop_distance, .. } = &self.state else {
            return None;
        };

        Some(*hop_distance)
    }

    pub fn notify_received_packet(&mut self, id: u8) -> Option<Timeout> {
        let State::Streaming {
            next_packet_id,
            packets_lost,
            packets_sent,
            ..
        } = &mut self.state
        else {
            return None;
        };

        let diff = Wrapping(id) - *next_packet_id;
        // TODO: handle past packets (wrong order) better (diff.0 will be < 128)
        debug_assert!(diff.0 < 128, "diff was {diff} (should be < 128)");
        let sent = u32::from(diff.0) + 1;
        *packets_sent = packets_sent.checked_add(sent).unwrap_or_else(|| {
            *packets_lost = 0;
            sent
        });
        *packets_lost += u32::from(diff.0);

        *next_packet_id += 1;

        Some((
            TimeoutId::Packet,
            self.config.message_period * (u32::from(self.config.gamma.get()) + 1),
        ))
    }
}

enum State {
    Startup,
    Construction {
        min_hop_distance: u16,
        position: glam::DVec3,
        joins_forwarded: FxHashSet<Ipv4Addr>,
    },
    AwaitingAreaInfo(Option<ForwardingJoinRequests>),
    Streaming {
        hop_distance: u16,
        area_info_id: Wrapping<u8>,
        nodes: FxHashMap<Ipv4Addr, message::NodeData>,
        network: Network,
        neighbours: Vec<Ipv4Addr>,
        parent: Ipv4Addr,
        next_packet_id: Wrapping<u8>,
        packets_lost: u32,
        packets_sent: u32,
    },
    AwaitingJoinAvailable {
        best: Option<ParentOption>,
    },
}

struct ForwardingJoinRequests {
    hop_distance: u16,
    joins_forwarded: FxHashSet<Ipv4Addr>,
}

struct ParentOption {
    address: Ipv4Addr,
    hop_distance: u16,
    confidence: f32,
}

// struct JoinGroupRequest {
//     child: Ipv4Addr,
//     min_hop_distance: u16,
// }

impl<N: NodeInfo> Area<N> {
    /// Handle an incomming control [`Message`](message::Message).
    ///
    #[doc = doc_handle_return!()]
    pub fn handle_message(&mut self, m: message::Message) -> Response {
        match m {
            message::Message::AreaConstruction(area_construction) => {
                self.handle_area_construction(area_construction)
            }
            message::Message::JoinReport(join_report) => self.handle_join_report(join_report),
            message::Message::AreaInfo(area_info) => self.handle_area_info(area_info),

            message::Message::JoinArea(join_area) => self.handle_join_area(join_area),
            message::Message::JoinAvailable(join_available) => {
                self.handle_join_available(join_available)
            }
            message::Message::JoinAccept(join_accept) => self.handle_join_accept(join_accept),
        }
    }

    /// Handle a timeout event.
    ///
    #[doc = doc_handle_return!()]
    pub fn handle_timeout(&mut self, id: TimeoutId) -> Response {
        match id {
            TimeoutId::Control => self.handle_control_timeout(),
            TimeoutId::Packet => self.handle_packet_timeout(),
        }
    }

    pub fn change_parent(&mut self, parent: Ipv4Addr) -> Option<message::Message> {
        // TODO: check that we are connected and in the new parent's RTR?

        let m = message::JoinAccept {
            address: self.address,
            position: self.node_info.position(),
            parent,
            forwarder: self.address,
        };
        Some(m.into())
    }

    fn handle_control_timeout(&mut self) -> Response {
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
                };

                self.state = State::AwaitingAreaInfo(Some(ForwardingJoinRequests {
                    hop_distance: *min_hop_distance,
                    joins_forwarded: mem::take(joins_forwarded),
                }));

                println!("{m:#?}");

                m.into()
            }

            State::AwaitingJoinAvailable { best } => {
                let Some(best) = best else {
                    // TODO: emit warning, change state?
                    // currently, keep waiting
                    return Default::default();
                };

                let m = message::JoinAccept {
                    address: self.address,
                    position: self.node_info.position(),
                    parent: best.address,
                    forwarder: self.address,
                };

                self.state = State::AwaitingAreaInfo(None);

                m.into()
            }

            _ => todo!("unexpected timeout; error? maybe only in debug"),
        }
    }

    fn handle_packet_timeout(&mut self) -> Response {
        self.state = State::AwaitingJoinAvailable { best: None };
        let m = message::JoinArea {
            address: self.address,
            position: self.node_info.position(),
        };
        let t = (TimeoutId::Control, self.config.construct_timeout);
        (m, t).into()
    }

    /// Handle an incomming [`AreaConstruction`](message::AreaConstruction) message.
    ///
    #[doc = doc_handle_return!()]
    pub fn handle_area_construction(&mut self, m: message::AreaConstruction) -> Response {
        match &mut self.state {
            State::Startup => {
                let position = self.node_info.position();

                // if either node is outside of the other's RTR, ignore it
                if position.distance_squared(m.position) > self.config.radius * self.config.radius {
                    return Default::default();
                }

                let ttl = m.ttl.get() - 1;
                debug_assert!(self.config.k.get() > ttl);

                self.state = State::Construction {
                    min_hop_distance: self.config.k.get() - ttl,
                    position,
                    joins_forwarded: FxHashSet::default(),
                };

                let m = NonZero::new(ttl).map(|ttl| message::AreaConstruction { ttl, position });
                (m, (TimeoutId::Control, self.config.construct_timeout)).into()
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
                };
                (m, (TimeoutId::Control, self.config.construct_timeout)).into()
            }

            _ => {
                // TODO: log error / warning?
                Default::default()
            }
        }
    }

    /// Handle an incomming [`JoinReport`](message::JoinReport) message.
    ///
    #[doc = doc_handle_return!()]
    pub fn handle_join_report(&mut self, mut m: message::JoinReport) -> Response {
        match &mut self.state {
            State::Startup | State::AwaitingJoinAvailable { .. } => {
                // TODO cache join requests to be sent later
                Default::default()
            }

            State::Construction {
                min_hop_distance: hop_distance,
                joins_forwarded,
                ..
            }
            | State::AwaitingAreaInfo(Some(ForwardingJoinRequests {
                hop_distance,
                joins_forwarded,
                ..
            })) => {
                if *hop_distance >= m.forwarder_hop_distance || joins_forwarded.contains(&m.address)
                {
                    return Default::default();
                }

                m.forwarder_hop_distance = *hop_distance;

                joins_forwarded.insert(m.address);

                m.into()
            }

            State::Streaming { .. } | State::AwaitingAreaInfo(None) => {
                // TODO: too late, log an error / warning
                Default::default()
            }
        }
    }

    /// Handle an incomming [`AreaInfo`](message::AreaInfo) message.
    ///
    #[doc = doc_handle_return!()]
    pub fn handle_area_info(&mut self, m: message::AreaInfo) -> Response {
        let message::AreaInfo { id, network, nodes } = m;
        let me = nodes.get(&self.address);

        let (mut neighbours, hop_distance) = match &mut self.state {
            State::Startup | State::Construction { .. } | State::AwaitingJoinAvailable { .. } => {
                println!(
                    "WARNING: NODE {} NOT IN AREA {} ({id})",
                    self.address, self.group
                );
                return Default::default();
            }

            State::AwaitingAreaInfo(Some(ForwardingJoinRequests { hop_distance, .. })) => {
                (Vec::new(), *hop_distance)
            }
            State::AwaitingAreaInfo(None) => {
                if let Some(me) = me {
                    let hop_distance = AncestorWalker::new(me.index).iter(&network).count();
                    (
                        Vec::new(),
                        u16::try_from(hop_distance).expect(
                            "expected the network to have a maximum hop distance of 65,535",
                        ),
                    )
                } else {
                    println!(
                        "WARNING: NODE {} NOT IN AREA {} ({id})",
                        self.address, self.group
                    );
                    return Default::default();
                }
            }

            State::Streaming {
                hop_distance,
                area_info_id,
                neighbours,
                ..
            } => {
                let diff = (m.id - *area_info_id).0;
                // if this is the current or an old version, ignore it
                // TODO: add constant to the config?
                if diff == 0 || diff > u8::MAX - 16 {
                    return Default::default();
                }

                if diff > 64 {
                    // TODO: WARNING, very high packet loss, can't reliably tell if this is new or old
                    todo!(
                        "potentially disconnect? (diff: {diff}, current: {area_info_id}, m: {:?})",
                        message::AreaInfo { id, network, nodes }
                    );
                }

                // TODO: add to packet loss counter?

                let mut neighbours = mem::take(neighbours);
                neighbours.clear();
                (neighbours, *hop_distance)
            }
        };

        let Some(me) = me else {
            println!(
                "WARNING: NODE {} NOT IN AREA {} ({id})",
                self.address, self.group
            );
            return Default::default();
        };
        neighbours.extend(network.neighbors(me.index).map(|i| network[i]));
        let mut parents = network.neighbors_directed(me.index, petgraph::Incoming);
        let parent = parents
            .next()
            .map(|i| network[i])
            .expect("expected to have a parent in the network");
        debug_assert!(
            parents.next().is_none(),
            "expected to have no more than one parent in the network"
        );

        let m = (!neighbours.is_empty()).then_some(message::AreaInfo {
            id,
            network: network.clone(),
            nodes: nodes.clone(),
        });

        self.state = State::Streaming {
            hop_distance,
            area_info_id: id,
            nodes,
            network,
            neighbours,
            parent,
            next_packet_id: Wrapping(0),
            packets_lost: 0,
            packets_sent: 0,
        };

        (m, Event::Parent(parent)).into()
    }

    pub fn handle_join_area(&mut self, m: message::JoinArea) -> Response {
        match &mut self.state {
            State::Startup
            | State::Construction { .. }
            | State::AwaitingAreaInfo { .. }
            | State::AwaitingJoinAvailable { .. } => Default::default(),

            State::Streaming {
                hop_distance,
                packets_lost,
                packets_sent,
                ..
            } => {
                debug_assert!(*hop_distance <= self.config.k.get());
                // are we allowed to forward?
                if *hop_distance == self.config.k.get() {
                    return Default::default();
                }

                // are we within RTR?
                let position = self.node_info.position();
                if position.distance_squared(m.position) > self.config.radius * self.config.radius {
                    return Default::default();
                }

                message::JoinAvailable {
                    address: m.address,
                    parent: self.address,
                    hop_distance: *hop_distance + 1,
                    #[expect(clippy::cast_possible_truncation)]
                    confidence: (1. - f64::from(*packets_lost) / f64::from(*packets_sent)) as f32,
                }
                .into()
            }
        }
    }

    pub fn handle_join_available(&mut self, m: message::JoinAvailable) -> Response {
        match &mut self.state {
            State::Startup
            | State::Construction { .. }
            | State::AwaitingAreaInfo { .. }
            | State::Streaming { .. } => Default::default(),

            State::AwaitingJoinAvailable { best } => {
                // if not for us, ignore it
                if m.address != self.address {
                    return Default::default();
                }

                if let Some(previous) = best {
                    // if the offer is no better, ignore it
                    match previous.hop_distance.cmp(&m.hop_distance) {
                        std::cmp::Ordering::Less => {
                            return Default::default();
                        }
                        std::cmp::Ordering::Equal if previous.confidence >= m.confidence => {
                            return Default::default();
                        }

                        _ => {}
                    }
                }

                *best = Some(ParentOption {
                    address: m.parent,
                    hop_distance: m.hop_distance,
                    confidence: m.confidence,
                });

                (TimeoutId::Control, self.config.construct_timeout).into()
            }
        }
    }

    pub fn handle_join_accept(&mut self, m: message::JoinAccept) -> Response {
        match &mut self.state {
            State::Startup
            | State::Construction { .. }
            | State::AwaitingAreaInfo { .. }
            | State::AwaitingJoinAvailable { .. } => Default::default(),

            State::Streaming { neighbours, .. } => {
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
                    "{} is forwarding JoinAccept from {}",
                    self.address, m.address
                );

                message::JoinAccept {
                    address: m.address,
                    position: m.position,
                    parent: m.parent,
                    forwarder: self.address,
                }
                .into()
            }
        }
    }
}
