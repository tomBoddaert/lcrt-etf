use std::num::NonZero;

use std::time;

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub k: NonZero<u16>,
    pub radius: f64,
    pub bitrate_capacity: f32,
    pub construct_timeout: time::Duration,
    pub source_construct_timeout: time::Duration,
}

impl Config {
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        let Self {
            k: _,
            radius,
            bitrate_capacity,
            construct_timeout,
            source_construct_timeout,
        } = self;

        radius.is_normal()
            && radius.is_sign_positive()
            && bitrate_capacity.is_normal()
            && bitrate_capacity.is_sign_positive()
            && !construct_timeout.is_zero()
            && !source_construct_timeout.is_zero()
    }
}
