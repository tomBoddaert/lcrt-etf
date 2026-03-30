use std::{hash::Hash, marker::PhantomData, ops::Index};

use glam::DVec3;
use petgraph::{
    adj::IndexType,
    csr::{self, Csr},
    visit::{GraphBase, IntoNodeIdentifiers},
};
use rustc_hash::FxHashMap;

use crate::{geo::Sphere, path::Path};

pub struct Intersections<'n, Id, G>
where
    &'n G: GraphBase,
{
    pub(crate) graph: Csr<(), f64, petgraph::Undirected, <&'n G as GraphBase>::NodeId>,
    // pub(crate) nodes: Vec<(Id, Sphere)>,
    network: &'n G,
    pub(crate) ixs: FxHashMap<Id, csr::NodeIndex<<&'n G as GraphBase>::NodeId>>,
}

impl<'n, Id, G> Intersections<'n, Id, G>
where
    &'n G: IntoNodeIdentifiers,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
    <&'n G as GraphBase>::NodeId: IndexType,
    Id: Copy + Eq + Hash,
{
    // pub fn new<N>(nodes: N) -> Self
    // where
    //     N: IntoIterator<Item = (Id, Sphere)>,
    pub fn new(network: &'n G) -> Self {
        let mut graph = Csr::new();
        // let mut new_nodes = Vec::new();
        let mut ixs = FxHashMap::default();

        for ixa in network.node_identifiers() {
            let (ida, a) = network[ixa];
            let ixa_2: <&'n G as GraphBase>::NodeId = graph.add_node(());
            debug_assert!(ixa_2 == ixa);
            ixs.insert(ida, ixa);

            graph
                .node_identifiers()
                .map(|ixb| (ixb, network[ixb].1))
                .filter_map(|(ixb, b)| a.intersection_distance(&b).map(|d| (ixb, d)))
                .for_each(|(ixb, d)| {
                    let added = graph.add_edge(ixa, ixb, d);
                    debug_assert!(added, "expected that the edge did not already exist");
                });
        }

        Self {
            graph,
            // nodes: new_nodes,
            network,
            ixs,
        }
    }

    pub fn get_ix(&self, id: &Id) -> <&'n G as GraphBase>::NodeId {
        self.ixs[id]
    }

    pub fn get_path(
        &self,
        start: <&'n G as GraphBase>::NodeId,
        b: DVec3,
    ) -> Option<Path<'n, Id, G>> {
        let (_, path) = petgraph::algo::astar(
            &self.graph,
            start,
            |ix| {
                let (_, s) = self.network[ix];
                s.contains(b)
            },
            |e| *e.weight(),
            |ix| {
                let (_, s) = self.network[ix];
                s.distance_to(b)
            },
        )?;

        Some(Path {
            // nodes: &self.nodes,
            network: self.network,
            path,
            _ids: PhantomData,
        })
    }
}
