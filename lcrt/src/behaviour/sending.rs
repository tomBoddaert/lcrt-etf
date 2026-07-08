use std::{net::Ipv4Addr, num::Wrapping};

use graph::Graph;

use crate::{Identifier, message::NodeData};

pub struct Sending<Id = Ipv4Addr> {
    children: Vec<Id>,
    next_packet_id: Wrapping<u8>,
}

impl<Id> Sending<Id>
where
    Id: Identifier,
{
    #[inline(always)]
    fn with_presets<E>(
        mut children_buffer: Vec<Id>,
        next_packet_id: Wrapping<u8>,
        network: &Graph<NodeData<Id>, E>,
        address: Id,
    ) -> Option<Self> {
        let n = network
            .nodes()
            .iter()
            .position(|node| node.address == address)?;

        children_buffer.extend(network.neighbours(n).map(|node| network.node(node).address));

        #[cfg(debug_assertions)]
        {
            let mut parents = network.reverse_neighbours(n);
            debug_assert!(
                parents.next().is_none(),
                "expected to have no parents in the network"
            );
        }

        Some(Self {
            children: children_buffer,
            next_packet_id,
        })
    }

    // TODO: figure out a way of removing this
    #[inline]
    pub const fn dummy() -> Self {
        Self {
            children: Vec::new(),
            next_packet_id: Wrapping(0),
        }
    }

    #[must_use]
    pub fn new<E>(network: &Graph<NodeData<Id>, E>, address: Id) -> Option<Self> {
        Self::with_presets(Vec::new(), Wrapping(0), network, address)
    }

    #[must_use]
    pub fn update<E>(self, network: &Graph<NodeData<Id>, E>, address: Id) -> Option<Self> {
        let Self {
            mut children,
            next_packet_id,
        } = self;
        children.clear();
        Self::with_presets(children, next_packet_id, network, address)
    }

    #[must_use]
    #[inline]
    pub const fn peek_next_packet_id(&self) -> Wrapping<u8> {
        self.next_packet_id
    }

    #[must_use]
    #[inline]
    pub fn next_packet_id(&mut self) -> Wrapping<u8> {
        let pid = self.next_packet_id;
        self.next_packet_id += 1;
        pid
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
