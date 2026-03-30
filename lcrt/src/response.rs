use std::{net::Ipv4Addr, time};

use crate::message::Message;

pub type Timeout = (TimeoutId, time::Duration);
// /// The response from `handle_*` functions.
// ///
// /// If a message is returned, it must be broadcast to neighbours.
// /// If a duration is returned, the area's `handle_timeout` must be called after that time. This **must override** any timers previously set by the area controller.
// pub type Response = (Option<message::Message>, Option<Timeout>);

#[derive(Clone, Debug, Default)]
pub struct Response {
    pub message: Option<Message>,
    pub timeout: Option<Timeout>,
    pub event: Option<Event>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum TimeoutId {
    Control = 1,
    Packet = 2,
}

impl From<TimeoutId> for u8 {
    #[inline]
    fn from(value: TimeoutId) -> Self {
        value as Self
    }
}

impl TryFrom<u8> for TimeoutId {
    type Error = (); // TODO: replace with error type

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Control),
            2 => Ok(Self::Packet),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Parent(Ipv4Addr),
}

macro_rules! ifty {
    (# m ) => { M };
    (# m o ) => { Option<M> };
    (# t ) => { Timeout };
    (# t o ) => { Option<Timeout> };
    (# e ) => { Event };
    (# e o ) => { Option<Event> };

    ( $( $t:ident $($o:ident)? ),* ) => {
        ($(
            ifty!(# $t $($o)? )
        ),*)
    };
}

macro_rules! ifs {
    (m: $val:ident $(, $_a:ident )* => m $(, $_b:ident $(o)? )* ) => {
        Some($val.into())
    };
    (m: $val:ident $(, $_a:ident )* => m o $(, $_b:ident $(o)? )* ) => {
        $val.map(Into::into)
    };

    (t: $val:ident $(, $_a:ident )* => t $(, $_b:ident $(o)? )* ) => {
        Some($val)
    };
    (t: $val:ident $(, $_a:ident )* => t o $(, $_b:ident $(o)? )* ) => {
        $val
    };

    (e: $val:ident $(, $_a:ident )* => e $(, $_b:ident $(o)? )* ) => {
        Some($val)
    };
    (e: $val:ident $(, $_a:ident )* => e o $(, $_b:ident $(o)? )* ) => {
        $val
    };

    ($f:tt: $_a:ident $(, $rem_a:ident )* => $_b:ident $(o)? $(, $rem_b:ident $( $o:ident )? )* ) => {
        ifs!($f: $( $rem_a ),* => $( $rem_b $($o)? ),*)
    };
    ($f:tt: => ) => {
        None
    };
}

macro_rules! impl_from {
    ( M ($( $t:ident $( $o:ident )? ),*) ) => {
        #[allow(clippy::allow_attributes, unused_parens)]
        impl<M> From<ifty!($( $t $($o)? ),*)> for Response
        where
            M: Into<Message>
        {
            #[inline]
            fn from(($( $t ),*): ifty!($( $t $($o)? ),*)) -> Self {
                Self {
                    message: ifs!(m: $( $t ),* => $( $t $($o)? ),*),
                    timeout: ifs!(t: $( $t ),* => $( $t $($o)? ),*),
                    event: ifs!(e: $( $t ),* => $( $t $($o)? ),*),
                }
            }
        }
    };

    ( ($( $t:ident $( $o:ident )? ),*) ) => {
        #[allow(clippy::allow_attributes, unused_parens)]
        impl From<ifty!($( $t $($o)? ),*)> for Response {
            #[inline]
            fn from(($( $t ),*): ifty!($( $t $($o)? ),*)) -> Self {
                Self {
                    message: ifs!(m: $( $t ),* => $( $t $($o)? ),*),
                    timeout: ifs!(t: $( $t ),* => $( $t $($o)? ),*),
                    event: ifs!(e: $( $t ),* => $( $t $($o)? ),*),
                }
            }
        }
    };

    ( $( $($m:ident)? ($( $t:ident $( $o:ident )? ),*) ),* ) => {
        $(
            impl_from!($($m)? ($( $t $($o)? ),*));
        )*
    };
}

impl_from! {
    M(m),
    M(m o),

    (t),
    (t o),

    (e),
    (e o),

    M(m, t),
    M(m o, t),
    M(m, t o),
    M(m o, t o),

    M(m, e),
    M(m o, e),
    M(m, e o),
    M(m o, e o),

    (t, e),
    (t o, e),
    (t, e o),
    (t o, e o),

    M(m, t, e),
    M(m o, t, e),
    M(m, t o, e),
    M(m, t, e o),
    M(m o, t o, e),
    M(m o, t, e o),
    M(m, t o, e o),
    M(m o, t o, e o)
}
