use std::ops::Index;

use common::AncestorWalker;
use glam::DVec3;
use petgraph::{
    csr::IndexType,
    visit::{GraphBase, IntoNeighborsDirected, Walker},
};

pub mod geo;
mod intersections;
mod path;
mod straight;
pub use crate::{intersections::Intersections, straight::get_straight_trajectory};

use crate::{geo::Sphere, path::Path};

#[must_use]
pub fn get_ancestor_path<'n, G, Id>(
    // nodes: &'n [(Id, Sphere)],
    network: &'n G,
    start: <&'n G as GraphBase>::NodeId,
    b: DVec3,
) -> Option<Path<'n, Id, G>>
where
    &'n G: IntoNeighborsDirected,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
    Id: Clone,
{
    let walker = AncestorWalker::new(start).iter(network);
    //     .map(|ix| {
    //     // network
    //     //     .node_weight(id)
    //     //     .expect("expected parent node to exist in network")
    //     &nodes[ix.index()]
    // });
    let (i, _) = walker.clone().enumerate().find(|(_, ix)| {
        network[*ix]
            // .node_weight(*ix)
            // .expect("expected parent node top exist in network")
            .1
            .contains(b)
    })?;

    // let mut nodes: Vec<(Id, Sphere)> = Vec::with_capacity(i + 2);
    // nodes.push(nodes[start.index()].clone());
    // nodes.extend(walker.take(i + 1).cloned());
    let mut path = Vec::with_capacity(i + 2);
    path.push(start);
    path.extend(walker.take(i + 1));

    Some(Path {
        // nodes: &[],
        network,
        path,
        _ids: std::marker::PhantomData,
    })
}
