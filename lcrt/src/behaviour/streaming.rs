use std::{net::Ipv4Addr, num::Wrapping};

use crate::Identifier;

pub struct Streaming<Id = Ipv4Addr> {
    parent: Id,
    children: Vec<Id>,
    next_packet_id: Wrapping<u8>,
    packets_lost: u32,
    packets_total: u32,
}

impl<Id> Streaming<Id>
where
    Id: Identifier,
{
    pub fn notify_received_packet(&mut self, id: u8) {
        let diff = Wrapping(id) - self.next_packet_id;
        // TODO: handle past packets (wrong order) better (diff.0 will be < 128)
        debug_assert!(diff.0 < 128, "diff was {diff} (should be < 128)");

        let sent = u32::from(diff.0) + 1;
        self.packets_total = self.packets_total.checked_add(sent).unwrap_or_else(|| {
            // reduce total but keep approximate proportion
            self.packets_lost /= u32::from(u16::MAX);
            self.packets_total / u32::from(u16::MAX) + sent
        });
        self.packets_lost += u32::from(diff.0);

        self.next_packet_id = Wrapping(id) + Wrapping(1);
    }

    #[must_use]
    #[inline]
    pub const fn get_parent(&self) -> Id {
        self.parent
    }

    #[must_use]
    #[inline]
    pub const fn get_children(&self) -> &[Id] {
        self.children.as_slice()
    }

    #[must_use]
    #[inline]
    pub const fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}
