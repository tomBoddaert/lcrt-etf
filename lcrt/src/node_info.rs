pub trait NodeInfo {
    fn position(&self) -> glam::DVec3;
    fn current_bitrate(&self) -> f32;
    fn interfering_neighbours(&self) -> u16;
}
