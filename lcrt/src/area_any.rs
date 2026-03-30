use std::net::Ipv4Addr;

use rustc_hash::FxHashMap;

use crate::{Area, AreaSource, Config, Network, NodeInfo, Response, Timeout, TimeoutId, message};

/// Routing controller for an LCRT area member.
///
/// Created from an [`Area`] or [`AreaSource`].
/// Can be created with [`Self::from`], [`Into<Self>::into`], or by constructing an enum variant.
pub enum AreaAny<N> {
    Area(Area<N>),
    AreaSource(AreaSource<N>),
}

impl<N: NodeInfo> From<Area<N>> for AreaAny<N> {
    #[inline]
    fn from(value: Area<N>) -> Self {
        Self::Area(value)
    }
}

impl<N: NodeInfo> From<AreaSource<N>> for AreaAny<N> {
    #[inline]
    fn from(value: AreaSource<N>) -> Self {
        Self::AreaSource(value)
    }
}

macro_rules! up_impl {
    {#body
        $ident:ident(&mut $self:ident $(, $arg:ident: $ty:ty )*)
    } => {
        match $self {
            Self::Area(area) => area.$ident($( $arg ),*),
            Self::AreaSource(area_source) => area_source.$ident($( $arg ),*),
        }
    };

    {#body
        $ident:ident(& $self:ident $(, $arg:ident: $ty:ty )*)
    } => {
        match $self {
            Self::Area(area) => area.$ident($( $arg ),*),
            Self::AreaSource(area_source) => area_source.$ident($( $arg ),*),
        }
    };

    {
        $(#[ $attr:meta ])*
        $vis:vis $([ $mod:ident ])* fn $ident:ident($( $args:tt )*) $(-> $return:ty)?;
    } => {
        $(#[ $attr ])*
        #[inline]
        $vis $( $mod )* fn $ident($( $args )*) $(-> $return)? {
            up_impl! {#body
                $ident($( $args )*)
            }
        }
    };

    {
        $(
            $(#[ $attr:meta ])*
            $vis:vis $([ $mod:ident ])* fn $ident:ident($( $args:tt )*) $(-> $return:ty)?;
        )+
    } => {
        $( up_impl! {
            $(#[ $attr ])*
            $vis $([ $mod ])* fn $ident($( $args )*) $(-> $return)?;
        } )+
    };
}

impl<N: NodeInfo> AreaAny<N> {
    up_impl! {
        /// Get the node's address.
        pub [const] fn get_address(&self) -> Ipv4Addr;
        /// Get the group address for the area.
        pub [const] fn get_group(&self) -> Ipv4Addr;
        pub [const] fn get_config(&self) -> &Config;
        pub [const] fn get_node_info(&self) -> &N;
        /// Returns whether this routing controller has established an area and is able to send/receive data streams.
        pub [const] fn is_streaming(&self) -> bool;
        /// If the network is established, returns the network topology graph and [`NodeData`](message::NodeData) map.
        pub [const] fn get_network(&self) -> Option<(&FxHashMap<Ipv4Addr, message::NodeData>, &Network)> ;
        /// If the network is established, returnss the node's children.
        pub [const] fn get_children(&self) -> Option<&[Ipv4Addr]>;
        /// Returns whether the network is established and the node has children (and is therefore a forwarder).
        pub [const] fn has_children(&self) -> bool;

        /// Handle a timeout event.
        ///
        #[doc = doc_handle_return!()]
        pub fn handle_message(&mut self, m: message::Message) -> Response;
        /// Handle an incomming control [`Message`](message::Message).
        ///
        #[doc = doc_handle_return!()]
        pub fn handle_timeout(&mut self, id: TimeoutId) -> Response;
    }

    #[inline]
    /// If the network is established and this is a non-source node, returns the node's parent.
    pub const fn get_parent(&self) -> Option<Ipv4Addr> {
        match self {
            Self::Area(area) => area.get_parent(),
            Self::AreaSource(_) => None,
        }
    }

    #[inline]
    /// If the network is established, returns the node's hop distance from the area source.
    pub fn get_hop_distance(&self) -> Option<u16> {
        match self {
            Self::Area(area) => area.get_hop_distance(),
            Self::AreaSource(area_source) => area_source.is_streaming().then_some(0),
        }
    }

    #[inline]
    /// If the node is a source node and the network is established, returns the next packet ID in the stream.
    pub fn next_packet_id(&mut self) -> Option<u8> {
        match self {
            Self::Area(_) => None,
            Self::AreaSource(area_source) => area_source.next_packet_id(),
        }
    }

    #[inline]
    pub fn notify_received_packet(&mut self, id: u8) -> Option<Timeout> {
        match self {
            Self::Area(area) => area.notify_received_packet(id),
            Self::AreaSource(_) => None,
        }
    }

    #[inline]
    pub fn change_parent(&mut self, parent: Ipv4Addr) -> Option<message::Message> {
        match self {
            Self::Area(area) => area.change_parent(parent),
            Self::AreaSource(_) => None,
        }
    }
}
