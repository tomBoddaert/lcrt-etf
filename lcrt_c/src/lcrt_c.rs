use std::{ffi::c_void, net::Ipv4Addr, num::NonZero, time};

use lcrt::message;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct LcrtPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<LcrtPosition> for glam::DVec3 {
    fn from(LcrtPosition { x, y, z }: LcrtPosition) -> Self {
        Self::new(x, y, z)
    }
}

impl From<glam::DVec3> for LcrtPosition {
    #[inline]
    fn from(glam::DVec3 { x, y, z }: glam::DVec3) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct LcrtConfig {
    /// Must be non-zero.
    pub k: u16,
    pub radius: f64,
    pub bitrate_capacity: f32,
    /// Nanoseconds.
    pub construct_timeout: u64,
    /// Nanoseconds.
    pub source_construct_timeout: u64,
    /// Nanoseconds.
    pub message_period: u64,
    /// Must be non-zero.
    pub gamma: u8,
}

#[unsafe(no_mangle)]
pub extern "C" fn lcrt_debug_config(config: LcrtConfig) {
    println!("{config:#?}");
}

impl From<LcrtConfig> for lcrt::Config {
    #[inline]
    fn from(
        LcrtConfig {
            k,
            radius,
            bitrate_capacity,
            construct_timeout,
            source_construct_timeout,
            message_period,
            gamma,
        }: LcrtConfig,
    ) -> Self {
        Self {
            k: NonZero::new(k).expect("k must be non-zero"),
            radius,
            bitrate_capacity,
            construct_timeout: time::Duration::from_nanos(construct_timeout),
            source_construct_timeout: time::Duration::from_nanos(source_construct_timeout),
            message_period: time::Duration::from_nanos(message_period),
            gamma: NonZero::new(gamma).expect("gamma must be non-zero"),
        }
    }
}

#[repr(C)]
pub struct LcrtNodeInfo {
    pub ctx: *mut c_void,
    pub position: unsafe extern "C" fn(ctx: *mut c_void) -> LcrtPosition,
    pub current_bitrate: unsafe extern "C" fn(ctx: *mut c_void) -> f32,
    pub interfering_neighbours: unsafe extern "C" fn(ctx: *mut c_void) -> u16,
}

impl lcrt::NodeInfo for LcrtNodeInfo {
    fn position(&self) -> glam::DVec3 {
        unsafe { (self.position)(self.ctx) }.into()
    }

    fn current_bitrate(&self) -> f32 {
        unsafe { (self.current_bitrate)(self.ctx) }
    }

    fn interfering_neighbours(&self) -> u16 {
        unsafe { (self.interfering_neighbours)(self.ctx) }
    }
}

// Wrapped in a 1-tuple to make the binding opaque.
pub type LcrtArea = (lcrt::AreaAny<LcrtNodeInfo>,);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_new(
    config: LcrtConfig,
    node: LcrtNodeInfo,
    address: u32,
    group: u32,
) -> *mut LcrtArea {
    // println!(
    //     "lcrt_area_new(address: {}, group: {})",
    //     std::net::Ipv4Addr::from(address),
    //     std::net::Ipv4Addr::from(group)
    // );
    let area = lcrt::Area::new(config.into(), node, address.into(), group.into());
    Box::into_raw(Box::new((area.into(),)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_source_new(
    config: LcrtConfig,
    node: LcrtNodeInfo,
    address: u32,
    group: u32,
    response: *mut LcrtResponse,
) -> *mut LcrtArea {
    // println!(
    //     "lcrt_area_source_new(address: {}, group: {})",
    //     std::net::Ipv4Addr::from(address),
    //     std::net::Ipv4Addr::from(group)
    // );
    let (area, r) = lcrt::AreaSource::new(config.into(), node, address.into(), group.into());
    unsafe { write_response(r, response) };

    Box::into_raw(Box::new((area.into(),)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_drop(area: *mut LcrtArea) {
    drop(unsafe { Box::from_raw(area) });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_handle_message(
    area: *mut LcrtArea,
    incoming: LcrtMessage,
    response: *mut LcrtResponse,
) {
    let m = unsafe { incoming.decode() }.unwrap();
    let r = unsafe { &mut *area }.0.handle_message(m);
    unsafe { write_response(r, response) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_handle_timeout(
    area: *mut LcrtArea,
    timeout_id: u8,
    response: *mut LcrtResponse,
) {
    let id = lcrt::TimeoutId::try_from(timeout_id).expect("expected timeout_id to be 1 or 2");
    let r = unsafe { &mut *area }.0.handle_timeout(id);
    unsafe { write_response(r, response) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_get_hop_distance(
    area: *const LcrtArea,
    hop_distance: *mut u16,
) -> bool {
    unsafe { &*area }
        .0
        .get_hop_distance()
        .map(|d| {
            unsafe { hop_distance.write(d) };
        })
        .is_some()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_is_forwarder(area: *const LcrtArea, dst: u32) -> bool {
    let area = &unsafe { &*area }.0;
    area.get_group() == Ipv4Addr::from(dst) && area.has_children()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_is_parent(area: *const LcrtArea, last_forwarder: u32) -> bool {
    let area = &unsafe { &*area }.0;
    area.get_parent()
        .is_some_and(|parent| parent == Ipv4Addr::from(last_forwarder))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_next_packet_id(area: *mut LcrtArea, id: *mut u8) -> bool {
    let area = &mut unsafe { &mut *area }.0;
    let Some(pid) = area.next_packet_id() else {
        return false;
    };

    unsafe { id.write(pid) };
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_notify_received_packet(
    area: *mut LcrtArea,
    id: u8,
    response: *mut LcrtResponse,
) {
    let area = &mut unsafe { &mut *area }.0;
    let t = area.notify_received_packet(id);
    unsafe { write_response(t.into(), response) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_area_change_parent(
    area: *mut LcrtArea,
    parent: u32,
    response: *mut LcrtResponse,
) {
    let area = &mut unsafe { &mut *area }.0;
    let m = area.change_parent(parent.into());
    unsafe { write_response(m.into(), response) };
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct LcrtMessage {
    pub data: *mut u8,
    pub len: usize,
}

pub const LCRT_MESSAGE_NULL: LcrtMessage = LcrtMessage {
    #[expect(
        clippy::zero_ptr,
        reason = "cbindgen can't generate a C constant with ptr::null_mut()"
    )]
    data: 0 as *mut u8,
    len: 0,
};

impl LcrtMessage {
    const fn from_slice(slice: *mut [u8]) -> Self {
        Self {
            data: slice.cast::<u8>(),
            len: slice.len(),
        }
    }

    fn from_box(slice: Box<[u8]>) -> Self {
        Self::from_slice(Box::into_raw(slice))
    }

    const unsafe fn as_slice(self) -> *mut [u8] {
        std::ptr::slice_from_raw_parts_mut(self.data, self.len)
    }

    unsafe fn into_box(self) -> Box<[u8]> {
        let slice = unsafe { self.as_slice() };
        unsafe { Box::from_raw(slice) }
    }

    unsafe fn decode(self) -> Option<message::Message> {
        let slice = unsafe { &*self.as_slice() };
        // println!("Decoding: {slice:x?}");
        // Some(ciborium::de::from_reader::<message::Message, _>(slice).unwrap())
        let msg = ciborium::de::from_reader::<message::Message, _>(slice).unwrap();
        // println!("Decoded {msg:?}");
        Some(msg)
    }

    fn encode(m: &message::Message) -> Self {
        // println!("Encoded {m:?}");
        let mut buffer = Vec::new();
        ciborium::ser::into_writer(m, &mut buffer).unwrap();
        // println!("Encoded: {buffer:x?}");
        let slice = Box::into_raw(buffer.into_boxed_slice());
        Self {
            data: slice.cast::<u8>(),
            len: slice.len(),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_message_new(len: usize) -> LcrtMessage {
    let buf = vec![0; len];
    LcrtMessage::from_box(buf.into_boxed_slice())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lcrt_message_drop(m: LcrtMessage) {
    drop(unsafe { LcrtMessage::into_box(m) });
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct LcrtTimeout {
    pub id: u8,
    pub delay: u64,
}

pub const LCRT_TIMEOUT_NULL: LcrtTimeout = LcrtTimeout { id: 0, delay: 0 };

impl From<lcrt::Timeout> for LcrtTimeout {
    fn from(value: lcrt::Timeout) -> Self {
        Self {
            id: value.0 as u8,
            delay: u64::try_from(value.1.as_nanos()).unwrap(),
        }
    }
}

impl From<Option<lcrt::Timeout>> for LcrtTimeout {
    #[inline]
    fn from(value: Option<lcrt::Timeout>) -> Self {
        value.map_or(LCRT_TIMEOUT_NULL, Self::from)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub enum LcrtEvent {
    #[default]
    None,
    Parent(u32),
}

impl From<Option<lcrt::Event>> for LcrtEvent {
    #[inline]
    fn from(value: Option<lcrt::Event>) -> Self {
        match value {
            None => Self::None,
            Some(lcrt::Event::Parent(parent)) => Self::Parent(parent.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct LcrtResponse {
    pub message: LcrtMessage,
    pub timeout: LcrtTimeout,
    pub event: LcrtEvent,
}

unsafe fn write_response(r: lcrt::Response, response: *mut LcrtResponse) {
    let lcrt::Response {
        message: m,
        timeout: t,
        event: e,
    } = r;

    let m = m.as_ref().map(LcrtMessage::encode).unwrap_or_default();
    unsafe { (&raw mut (*response).message).write(m) };
    unsafe { (&raw mut (*response).timeout).write(t.into()) };
    unsafe { (&raw mut (*response).event).write(e.into()) };
}
