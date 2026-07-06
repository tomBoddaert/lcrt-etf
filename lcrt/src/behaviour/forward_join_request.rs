use std::net::Ipv4Addr;

use rustc_hash::FxHashSet;

use crate::{Identifier, message};

pub struct ForwardJoinRequests<Id = Ipv4Addr> {
    forwarded: FxHashSet<Id>,
}

impl<Id> ForwardJoinRequests<Id>
where
    Id: Identifier,
{
    #[inline]
    pub fn new() -> Self {
        Self {
            forwarded: FxHashSet::default(),
        }
    }

    pub fn handle_join_report(
        &mut self,
        mut m: message::JoinReport<Id>,
        hop_distance: u16,
    ) -> Option<message::JoinReport<Id>> {
        if hop_distance >= m.hop_distance || self.forwarded.contains(&m.address) {
            return None;
        }

        m.forwarder_hop_distance = hop_distance;
        self.forwarded.insert(m.address);

        Some(m)
    }
}
