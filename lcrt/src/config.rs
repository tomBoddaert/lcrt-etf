use std::{num::NonZero, time::Duration};

#[derive(Copy, Clone, Debug)]
/// The LCRT area configuration.
pub struct Config {
    /// Maximum allowed number of hops in the network.
    pub k: NonZero<u16>,
    /// Node reliable coverage radius.
    ///
    /// The units are arbitrary, but must be consistent with [`NodeInfo::position`](crate::NodeInfo::position) and form a 3D Euclidean space.
    pub radius: f64,
    /// The maximum possible total transmission bitrate.
    ///
    /// Value must be in bits per second (bit/s).
    pub bitrate_capacity: f32,
    /// The duration to wait between finding a better parent node and commiting to it.
    ///
    /// This **must** be non-zero.
    pub construct_timeout: Duration,
    /// The duration to wait between advertising a new area and constructing the network.
    ///
    /// This **must** be greater than [`Self::construct_timeout`] and should scale with [`Self::k`].
    pub source_construct_timeout: Duration,
    pub message_period: Duration,
    pub gamma: NonZero<u8>,
}

impl Config {
    #[must_use]
    /// Check the config's validity.
    ///
    /// Returns whether all of the following apply:
    /// - [`Self::radius`] is normal (see [`f64::is_normal`]) with positive sign (see [`f64::is_sign_positive`]).
    /// - [`Self::bitrate_capacity`] is normal (see [`f32::is_normal`]) with positive sign (see [`f32::is_sign_positive`]).
    /// - [`Self::construct_timeout`] is positive (see [`Duration::is_zero`]).
    /// - [`Self::source_construct_timeout`] is greater than [`Self::construct_timeout`].
    /// - [`Self::message_period`] is positive (see [`Duration::is_zero`]).
    /// - [`Self::gamma`] is less than `128`.
    pub const fn is_valid(&self) -> bool {
        let Self {
            k: _,
            radius,
            bitrate_capacity,
            construct_timeout,
            source_construct_timeout,
            message_period,
            gamma,
        } = self;

        radius.is_normal()
            && radius.is_sign_positive()
            && bitrate_capacity.is_normal()
            && bitrate_capacity.is_sign_positive()
            && !construct_timeout.is_zero()
            && !source_construct_timeout
                .saturating_sub(*construct_timeout)
                .is_zero() // source_construct_timeout < construct_timeout (implemented this way for const)
            && !message_period.is_zero()
            && gamma.get() < 128
    }
}
