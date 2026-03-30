use petgraph::visit::{IntoNeighborsDirected, Walker};

#[derive(Clone, Copy, Debug)]
pub struct AncestorWalker<Ix> {
    node: Ix,
}

impl<Ix> AncestorWalker<Ix>
where
    Ix: Copy + PartialEq,
{
    #[inline]
    pub const fn new(node_index: Ix) -> Self {
        Self { node: node_index }
    }
}

impl<G> Walker<G> for AncestorWalker<G::NodeId>
where
    G: IntoNeighborsDirected,
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
