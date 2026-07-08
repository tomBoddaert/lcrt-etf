use std::{mem, net::Ipv4Addr, num::Wrapping};

use common::geo::Sphere;
use graph::Graph;
use rustc_hash::FxHashSet;

use crate::{
    Config, Event, Network, NodeInfo, Response, Timeout, TimeoutId,
    behaviour::{Construction, ForwardJoinRequests, Streaming},
    message,
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
    pub const fn get_network(&self) -> Option<&Network> {
        let State::Streaming { network, .. } = &self.state else {
            return None;
        };

        Some(network)
    }

    #[inline]
    /// If the network is established, returns the node's parent.
    pub const fn get_parent(&self) -> Option<Ipv4Addr> {
        let State::Streaming { streaming, .. } = &self.state else {
            return None;
        };

        Some(streaming.get_parent())
    }

    #[inline]
    /// If the network is established, returnss the node's children.
    pub const fn get_children(&self) -> Option<&[Ipv4Addr]> {
        let State::Streaming { streaming, .. } = &self.state else {
            return None;
        };

        Some(streaming.get_children())
    }

    #[inline]
    /// Returns whether the network is established and the node has children (and is therefore a forwarder).
    pub const fn has_children(&self) -> bool {
        let State::Streaming { streaming, .. } = &self.state else {
            return false;
        };

        streaming.has_children()
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
        let State::Streaming { streaming, .. } = &mut self.state else {
            return None;
        };

        streaming.notify_received_packet(id);

        Some((
            TimeoutId::Packet,
            self.config.message_period * (u32::from(self.config.gamma.get()) + 1),
        ))
    }

    #[must_use]
    #[inline]
    fn position(&self) -> Sphere {
        Sphere::new(self.node_info.position(), self.config.radius)
    }
}

enum State {
    Startup,
    Construction {
        construction: Construction,
        forward_joins: ForwardJoinRequests,
    },
    AwaitingAreaInfo(Option<(ForwardJoinRequests, u16)>),
    Streaming {
        hop_distance: u16,
        area_info_id: Wrapping<u8>,
        network: Network,
        streaming: Streaming,
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

impl<N: NodeInfo> Area<N> {
    /// Handle an incoming control [`Message`](message::Message).
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
            position: self.position(),
            parent,
            forwarder: self.address,
        };
        Some(m.into())
    }

    fn handle_control_timeout(&mut self) -> Response {
        match &mut self.state {
            State::Construction {
                construction,
                forward_joins,
            } => {
                let m = construction.handle_control_timeout(
                    self.address,
                    self.config.bitrate_capacity,
                    self.node_info.current_bitrate(),
                    self.node_info.interfering_neighbours(),
                );

                self.state =
                    State::AwaitingAreaInfo(Some((mem::take(forward_joins), m.hop_distance)));

                println!("{m:#?}");

                m.into()
            }

            State::AwaitingJoinAvailable { best } => {
                let Some(best) = best else {
                    // TODO: emit warning, change state?
                    // currently, keep waiting
                    return Default::default();
                };

                let parent = best.address;
                let m = message::JoinAccept {
                    address: self.address,
                    position: self.position(),
                    parent,
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

    /// Handle an incoming [`AreaConstruction`](message::AreaConstruction) message.
    ///
    #[doc = doc_handle_return!()]
    pub fn handle_area_construction(&mut self, m: message::AreaConstruction) -> Response {
        match &mut self.state {
            State::Startup => {
                let position = self.position();
                let Some((construction, m)) = Construction::new(m, position) else {
                    return Response::default();
                };

                self.state = State::Construction {
                    construction,
                    forward_joins: ForwardJoinRequests::new(),
                };

                (m, (TimeoutId::Control, self.config.construct_timeout)).into()
            }

            State::Construction { construction, .. } => {
                let Some(m) = construction.handle_area_construction(m) else {
                    return Response::default();
                };

                (m, (TimeoutId::Control, self.config.construct_timeout)).into()
            }

            _ => {
                // TODO: log error / warning?
                Default::default()
            }
        }
    }

    /// Handle an incoming [`JoinReport`](message::JoinReport) message.
    ///
    #[doc = doc_handle_return!()]
    pub fn handle_join_report(&mut self, m: message::JoinReport) -> Response {
        match &mut self.state {
            State::Startup | State::AwaitingJoinAvailable { .. } => {
                // TODO cache join requests to be sent later
                Default::default()
            }

            State::Construction {
                construction,
                forward_joins,
            } => forward_joins
                .handle_join_report(m, construction.get_hop_distance())
                .into(),

            State::AwaitingAreaInfo(Some((forward_joins, hop_distance))) => {
                forward_joins.handle_join_report(m, *hop_distance).into()
            }

            State::Streaming { .. } | State::AwaitingAreaInfo(None) => {
                // TODO: too late, log an error / warning
                Default::default()
            }
        }
    }

    /// Handle an incoming [`AreaInfo`](message::AreaInfo) message.
    ///
    #[doc = doc_handle_return!()]
    pub fn handle_area_info(&mut self, m: message::AreaInfo) -> Response {
        let Some((streaming, hop_distance)) = (match &mut self.state {
            State::Startup | State::Construction { .. } | State::AwaitingJoinAvailable { .. } => {
                println!(
                    "WARNING: NODE {} NOT IN AREA {} ({}, never sent JOIN_REPORT)",
                    self.address, self.group, m.id
                );
                return Response::default();
            }

            State::AwaitingAreaInfo(Some((_, hop_distance))) => {
                Streaming::new(&m, self.address).map(|s| (s, Some(*hop_distance)))
            }
            State::AwaitingAreaInfo(None) => Streaming::new(&m, self.address).map(|s| (s, None)),

            State::Streaming { area_info_id, .. } => 'update_streaming: {
                let diff = (m.id - *area_info_id).0;
                // if this is the current or an old version, ignore it
                // TODO: add constant to the config?
                if diff == 0 || diff > u8::MAX - 16 {
                    return Response::default();
                }

                if diff > 64 {
                    // TODO: WARNING, very high packet loss, can't reliably tell if this is new or old
                    todo!(
                        "potentially disconnect? (diff: {diff}, current: {area_info_id})", //, m: {m:?})",
                    );
                    // break 'update_streaming None; // ?
                }

                // TODO: add to packet loss counter?

                let State::Streaming {
                    hop_distance,
                    streaming,
                    ..
                } = mem::replace(&mut self.state, State::AwaitingAreaInfo(None))
                else {
                    unreachable!();
                };
                let Some(streaming) = streaming.update(&m, self.address) else {
                    break 'update_streaming None;
                };

                Some((streaming, Some(hop_distance)))
            }
        }) else {
            self.state = State::AwaitingAreaInfo(None);
            println!(
                "WARNING: NODE {} NOT IN AREA {} ({})",
                self.address, self.group, m.id
            );
            return Response::default();
        };

        let hop_distance = hop_distance.unwrap_or_else(|| {
            calculate_depth(&m.network, |n| &n.address, &self.address)
                .expect("expected to be in network (this should have already been checked)")
        });

        let m_forward = (streaming.has_children()).then(|| m.clone());
        let e = Event::Parent(streaming.get_parent());

        self.state = State::Streaming {
            hop_distance,
            area_info_id: m.id,
            network: m.network,
            streaming,
        };

        (m_forward, e).into()
    }

    pub fn handle_join_area(&mut self, m: message::JoinArea) -> Response {
        match &mut self.state {
            State::Startup
            | State::Construction { .. }
            | State::AwaitingAreaInfo { .. }
            | State::AwaitingJoinAvailable { .. } => Default::default(),

            State::Streaming {
                hop_distance,
                streaming,
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
                    confidence: streaming.get_confidence() as f32,
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

            State::Streaming { streaming, .. } => {
                // if we are not the parent and we are not the next forwarder, ignore it
                if m.forwarder == m.address {
                    if m.parent != self.address {
                        return Response::default();
                    }
                    // TODO: temporarily add the node to the neighbours to start forwarding immediately
                    // need to also accept messages on the other end
                } else if !streaming.get_children().contains(&m.forwarder) {
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

fn calculate_depth<N, E, F, I>(tree: &Graph<N, E>, mut key: F, node: &I) -> Option<u16>
where
    F: FnMut(&N) -> &I,
    I: ?Sized + Eq,
{
    #[inline]
    fn get_parent<N, E>(tree: &Graph<N, E>, i: usize) -> Option<usize> {
        let mut parents = tree.reverse_neighbours(i);
        let parent = parents.next();
        debug_assert!(
            parents.next().is_none(),
            "expected node in tree to have at most one parent"
        );
        parent
    }

    let mut i = tree.nodes().iter().position(|n| key(n) == node)?;
    let mut depth = 0;
    while let Some(p) = get_parent(tree, i) {
        depth += 1;
        i = p;
    }

    Some(depth)
}

#[cfg(test)]
mod test {
    use crate::{
        area::calculate_depth,
        test::tree_example::{NODES, node_info_tree},
    };

    #[test]
    fn depth_calculation() {
        let tree = node_info_tree();

        for node in NODES {
            let depth = calculate_depth(&tree, |n| n.id, node.id);
            assert_eq!(depth, Some(node.hop_distance));
        }
    }
}
