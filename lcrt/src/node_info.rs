/// An interface to retrieve information about the state of the node needed for routing.
pub trait NodeInfo {
    /// Returns the position of the node.
    ///
    /// The units are arbitrary, but must be consistent with [`Config::radius`](crate::Config::radius) and form a 3D Euclidean space.
    fn position(&self) -> glam::DVec3;
    /// Returns the current bitrate being transmitted.
    ///
    /// Value must be in bits per second (bit/s).
    fn current_bitrate(&self) -> f32;
    /// Returns the number of nodes transmitting data within interference range.
    fn interfering_neighbours(&self) -> u16;
}
