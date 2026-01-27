use glam::DVec3;
use petgraph::{
    csr,
    visit::{self, GraphBase, Walker},
};

pub mod geo;
pub mod intersections;
mod straight;
pub use intersections::Intersections;
pub use straight::get_straight_trajectory;

use crate::{
    geo::{Line, Sphere},
    intersections::{Path, PathIterator},
};

#[derive(Clone, Copy, Debug)]
pub struct AncestorWalker<Id> {
    node: Id,
}

impl<G> Walker<G> for AncestorWalker<G::NodeId>
where
    G: GraphBase + visit::IntoNeighborsDirected,
{
    type Item = G::NodeId;

    fn walk_next(&mut self, context: G) -> Option<Self::Item> {
        let mut neighbours = context.neighbors_directed(self.node, petgraph::Direction::Incoming);
        self.node = neighbours.next()?;
        debug_assert!(
            neighbours.next().is_none(),
            "expected the network to be a tree"
        );
        Some(self.node)
    }
}

pub fn get_ancestor_path<G, Id>(
    network: G,
    connected: G::NodeId,
    b: DVec3,
) -> Option<Vec<(Id, Sphere)>>
where
    G: visit::IntoNeighborsDirected
        + petgraph::data::DataMap
        + visit::Data<NodeWeight = (Id, Sphere), EdgeWeight = ()>,
    Id: Clone,
{
    let walker = AncestorWalker { node: connected }.iter(network).map(|id| {
        network
            .node_weight(id)
            .expect("expected parent node to exist in network")
    });
    let (i, _) = walker
        .clone()
        .enumerate()
        .find(|(_, (_, s))| s.contains(b))?;

    Some(walker.take(i + 1).cloned().collect())
}
