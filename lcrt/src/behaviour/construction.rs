use std::num::NonZero;

use common::geo::Sphere;

use crate::message;

pub struct Construction {
    min_hop_distance: u16,
    position: Sphere,
}

impl Construction {
    pub fn new(
        m: message::AreaConstruction,
        position: Sphere,
    ) -> Option<(Self, Option<message::AreaConstruction>)> {
        // if either node is outside of the other's RTR, ignore it
        if !position.mutual_contains_origin(&m.position) {
            return None;
        }

        let ttl = m.ttl.get() - 1;
        // TODO(resilience): handle ttl > k
        let hop_distance = m.k.get().checked_sub(ttl).expect("expected ttl <= k");

        Some((
            Self {
                min_hop_distance: hop_distance,
                position,
            },
            NonZero::new(ttl).map(|ttl| message::AreaConstruction {
                k: m.k,
                ttl,
                position,
            }),
        ))
    }

    pub fn handle_area_construction(
        &mut self,
        m: message::AreaConstruction,
    ) -> Option<message::AreaConstruction> {
        // if either node is outside of the other's RTR, ignore it
        if !self.position.mutual_contains_origin(&m.position) {
            return None;
        }

        let ttl = m.ttl.get() - 1;
        // TODO(resilience): handle ttl > k
        let hop_distance = m.k.get().checked_sub(ttl).expect("expected ttl <= k");

        // if the hop distance is no better, ignore it
        if hop_distance >= self.min_hop_distance {
            return None;
        }

        // TODO: handle error
        // assuming k has stayed constant, hd < mhd, so ttl > maxttl >= 0
        // if this fails, then k must have changed
        let ttl = NonZero::new(ttl).expect("expected improved ttl to be nonzero");
        self.min_hop_distance = hop_distance;

        Some(message::AreaConstruction {
            k: m.k,
            ttl,
            position: self.position,
        })
    }
}
