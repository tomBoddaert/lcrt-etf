use std::net::Ipv4Addr;

use rustc_hash::FxHashMap;

use crate::{Area, AreaSource, Network, NodeInfo, Response, message};

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
        pub [const] fn is_streaming(&self) -> bool;
        pub [const] fn get_network(&self) -> Option<(&FxHashMap<Ipv4Addr, message::NodeData>, &Network)> ;
        pub fn get_next_hops(&self, dst: Ipv4Addr) -> (&[Ipv4Addr], bool);
        pub fn is_forwarder(&self, dst: Ipv4Addr) -> bool;

        pub fn handle_message(&mut self, m: message::Message) -> Response;
        pub fn handle_timeout(&mut self) -> Response;
    }

    #[inline]
    pub fn is_parent(&self, last_forwarder: Ipv4Addr) -> bool {
        match self {
            Self::Area(area) => area.is_parent(last_forwarder),
            Self::AreaSource(_) => false,
        }
    }

    #[inline]
    pub fn get_hop_distance(&self) -> Option<u16> {
        match self {
            Self::Area(area) => area.get_hop_distance(),
            Self::AreaSource(area_source) => area_source.is_streaming().then_some(0),
        }
    }
}
