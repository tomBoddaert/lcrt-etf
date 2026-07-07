use std::{net::Ipv4Addr, num::Wrapping};

use crate::{Identifier, message};

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
    #[inline(always)]
    fn with_presets(
        mut children_buffer: Vec<Id>,
        packets_lost: u32,
        packets_total: u32,
        m: &message::AreaInfo<Id>,
        address: Id,
    ) -> Option<Self> {
        let n = m
            .network
            .nodes()
            .iter()
            .position(|node| node.address == address)?;

        children_buffer.extend(m.network.neighbours(n).map(|i| m.network.node(i).address));

        let mut parents = m.network.reverse_neighbours(n);
        let parent = parents
            .next()
            .map(|i| m.network.node(i).address)
            .expect("expected to have a parent in the network");
        debug_assert!(
            parents.next().is_none(),
            "expected to have no more than one parent in the network"
        );

        Some(Self {
            parent,
            children: children_buffer,
            next_packet_id: m.next_packet_id,
            packets_lost,
            packets_total,
        })
    }

    pub fn new(m: &message::AreaInfo<Id>, address: Id) -> Option<Self> {
        Self::with_presets(Vec::new(), 0, 0, m, address)
    }

    pub fn update(self, m: &message::AreaInfo<Id>, address: Id) -> Option<Self> {
        let Self {
            mut children,
            packets_lost,
            packets_total,
            ..
        } = self;
        children.clear();
        Self::with_presets(children, packets_lost, packets_total, m, address)
    }

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

    #[must_use]
    pub fn get_confidence(&self) -> f64 {
        if self.packets_total == 0 {
            0.5
        } else {
            1. - f64::from(self.packets_lost) / f64::from(self.packets_total)
        }
    }
}
