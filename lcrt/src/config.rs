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
