// TODO: warn on unwrap

use std::{hash::Hash, num::NonZero};

pub(crate) mod area;
pub mod message;
pub mod node;
pub(crate) mod source;

const BUFFER_LEN: usize = 3;

pub trait Address: Copy + Eq + Hash + Send + Sync + 'static {}
impl<T: Copy + Eq + Hash + Send + Sync + 'static> Address for T {}

pub struct Config {
    pub k: NonZero<u16>,
    pub construct_timeout: std::time::Duration,
    pub source_construct_timeout: std::time::Duration,
}

// pub trait Backoff {
//     fn reset(&mut self) -> std::time::Duration;
// }

// pub trait BackoffBuilder {
//     type Backoff: Backoff;

//     fn build(&self) -> (Self::Backoff, std::time::Duration);
// }

// pub struct ConstantBackoff {
//     duration: std::time::Duration,
// }

// pub struct ConstantBackoffBuilder {
//     pub duration: std::time::Duration,
// }

// impl Backoff for ConstantBackoff {
//     #[inline]
//     fn reset(&mut self) -> std::time::Duration {
//         self.duration
//     }
// }

// impl BackoffBuilder for ConstantBackoffBuilder {
//     type Backoff = ConstantBackoff;

//     #[inline]
//     fn build(&self) -> (Self::Backoff, std::time::Duration) {
//         (
//             Self::Backoff {
//                 duration: self.duration,
//             },
//             self.duration,
//         )
//     }
// }

// impl ConstantBackoffBuilder {
//     #[inline]
//     pub const fn new(duration: std::time::Duration) -> Self {
//         Self { duration }
//     }

//     #[inline]
//     pub const fn ms30() -> Self {
//         Self {
//             duration: std::time::Duration::from_millis(30),
//         }
//     }
// }
