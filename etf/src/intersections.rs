use std::{
    hash::Hash,
    iter::{self, FusedIterator},
    slice::{self, Windows},
};

use glam::DVec3;
use petgraph::{
    csr::{self, Csr},
    visit::IntoNodeIdentifiers,
};
use rustc_hash::FxHashMap;

use crate::geo::{Line, Sphere};

pub struct Intersections<Id, Ix = csr::DefaultIx> {
    pub(crate) graph: Csr<(), f64, petgraph::Undirected, Ix>,
    pub(crate) nodes: Vec<(Id, Sphere)>,
    pub(crate) ixs: FxHashMap<Id, csr::NodeIndex<Ix>>,
}

impl<Id, Ix> Intersections<Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
    Id: Clone + Eq + Hash,
{
    pub fn new<N>(nodes: N) -> Self
    where
        N: IntoIterator<Item = (Id, Sphere)>,
    {
        let mut graph = Csr::new();
        let mut new_nodes = Vec::new();
        let mut ixs = FxHashMap::default();

        for node in nodes {
            let (ida, a) = node;
            let ixa = graph.add_node(());
            debug_assert_eq!(usize::from(ixa), new_nodes.len());
            new_nodes.push((ida.clone(), a));
            ixs.insert(ida, ixa);

            graph
                .node_identifiers()
                .map(|ixb| (ixb, new_nodes[usize::from(ixb)].1))
                .filter_map(|(ixb, b)| a.intersection_distance(&b).map(|d| (ixb, d)))
                .for_each(|(ixb, d)| {
                    let added = graph.add_edge(ixa, ixb, d);
                    debug_assert!(added, "expected that the edge did not already exist");
                });

            for ixb in graph.node_identifiers() {
                let (_, b) = new_nodes[usize::from(ixb)];
                let Some(d) = a.intersection_distance(&b) else {
                    continue;
                };

                graph.add_edge(ixa, ixb, d);
            }
        }

        Self {
            graph,
            nodes: new_nodes,
            ixs,
        }
    }
}

pub struct Path<'a, Id, Ix = csr::DefaultIx> {
    pub(crate) intersections: &'a Intersections<Id, Ix>,
    pub(crate) path: Vec<csr::NodeIndex<Ix>>,
    pub(crate) cost: f64,
}

impl<Id, Ix> Intersections<Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
    Id: Clone + Eq + Hash,
{
    pub fn get_path(&self, start: csr::NodeIndex<Ix>, b: DVec3) -> Option<Path<'_, Id, Ix>> {
        let (cost, path) = petgraph::algo::astar(
            &self.graph,
            start,
            |ix| {
                let (_, s) = self.nodes[usize::from(ix)];
                s.contains(b)
            },
            |e| *e.weight(),
            |ix| {
                let (_, s) = self.nodes[usize::from(ix)];
                s.distance_to(b)
            },
        )?;

        Some(Path {
            intersections: self,
            path,
            cost,
        })
    }
}

impl<Id, Ix> Path<'_, Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
{
    #[must_use]
    pub fn iter(&self) -> PathIterator<'_, Id, Ix> {
        PathIterator {
            nodes: &self.intersections.nodes,
            path: self.path.iter().copied(),
        }
    }

    #[must_use]
    #[inline]
    pub fn segments(&self, a: DVec3) -> Segments<'_, Id, Ix> {
        Segments {
            nodes: &self.intersections.nodes,
            edges: self.path.windows(2),
            a,
        }
    }
}

impl<'a, Id, Ix> IntoIterator for &'a Path<'a, Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
{
    type Item = <Self::IntoIter as Iterator>::Item;
    type IntoIter = PathIterator<'a, Id, Ix>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct PathIterator<'a, Id, Ix = csr::DefaultIx> {
    nodes: &'a [(Id, Sphere)],
    path: iter::Copied<slice::Iter<'a, Ix>>,
}

impl<'a, Id, Ix> Iterator for PathIterator<'a, Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
{
    type Item = &'a (Id, Sphere);

    fn next(&mut self) -> Option<Self::Item> {
        self.path.next().map(usize::from).map(|n| &self.nodes[n])
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.path.nth(n).map(usize::from).map(|n| &self.nodes[n])
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<Id, Ix> ExactSizeIterator for PathIterator<'_, Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
{
    #[inline]
    fn len(&self) -> usize {
        self.path.len()
    }
}

impl<Id, Ix> DoubleEndedIterator for PathIterator<'_, Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.path
            .next_back()
            .map(usize::from)
            .map(|n| &self.nodes[n])
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.path
            .nth_back(n)
            .map(usize::from)
            .map(|n| &self.nodes[n])
    }
}

impl<Id, Ix> FusedIterator for PathIterator<'_, Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
{
}

#[must_use]
fn find_next_point(a: DVec3, fa: &Sphere, fb: &Sphere) -> DVec3 {
    let o = fa.intersection_midpoint(fb);
    let line = Line::new(a, o);
    let mb = line.sphere_intersection(fb).get_first_unchecked();
    line.interpolate(mb)
}

pub struct Segments<'a, Id, Ix = csr::DefaultIx> {
    nodes: &'a [(Id, Sphere)],
    edges: Windows<'a, Ix>,
    a: DVec3,
}

impl<'a, Id, Ix> Iterator for Segments<'a, Id, Ix>
where
    Ix: petgraph::adj::IndexType,
    usize: From<Ix>,
{
    type Item = (DVec3, &'a Id);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let [a, b] = self.edges.next()? else {
            unreachable!("expected windows to be of length 2");
        };
        let (_, fa) = &self.nodes[usize::from(*a)];
        let (id, fb) = &self.nodes[usize::from(*b)];

        self.a = find_next_point(self.a, fa, fb);
        Some((self.a, id))
    }
}
