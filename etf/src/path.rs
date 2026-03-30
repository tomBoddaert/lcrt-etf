use std::{
    iter::{self, FusedIterator},
    marker::PhantomData,
    ops::Index,
    slice::{self, Windows},
};

use glam::DVec3;
use petgraph::{
    csr,
    visit::{GraphBase, IntoNeighborsDirected},
};

use crate::geo::{Line, Sphere};

pub struct Path<'n, Id, G>
where
    &'n G: GraphBase,
{
    // pub(crate) nodes: Ns,
    pub(crate) network: &'n G,
    pub(crate) path: Vec<csr::NodeIndex<<&'n G as GraphBase>::NodeId>>,
    pub(crate) _ids: PhantomData<[Id]>,
}

impl<'n, Id, G> Path<'n, Id, G>
where
    &'n G: IntoNeighborsDirected,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
    Id: Copy,
{
    #[must_use]
    #[inline]
    pub fn iter(&self) -> PathIterator<'_, 'n, Id, G> {
        PathIterator {
            network: self.network,
            path: self.path.iter().copied(),
            _id: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    pub fn segments(&self, a: DVec3) -> Segments<'_, 'n, Id, G> {
        Segments {
            network: self.network,
            edges: self.path.windows(2),
            a,
            _id: PhantomData,
        }
    }
}

impl<'a, 'n, Id, G> IntoIterator for &'a Path<'n, Id, G>
where
    Id: Copy + 'n,
    &'n G: IntoNeighborsDirected,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
{
    type Item = <Self::IntoIter as Iterator>::Item;
    type IntoIter = PathIterator<'a, 'n, Id, G>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct PathIterator<'a, 'n, Id, G>
where
    &'n G: GraphBase,
{
    network: &'n G,
    path: iter::Copied<slice::Iter<'a, <&'n G as GraphBase>::NodeId>>,
    _id: PhantomData<[Id]>,
}

impl<'n, Id, G> Iterator for PathIterator<'_, 'n, Id, G>
where
    Id: Copy + 'n,
    &'n G: IntoNeighborsDirected,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
{
    type Item = &'n (Id, Sphere);

    fn next(&mut self) -> Option<Self::Item> {
        self.path.next().as_ref().copied().map(|n| &self.network[n])
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.path.nth(n).as_ref().copied().map(|n| &self.network[n])
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, 'n, Id, G> ExactSizeIterator for PathIterator<'a, 'n, Id, G>
where
    Id: Copy + 'n,
    &'n G: IntoNeighborsDirected,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
{
    #[inline]
    fn len(&self) -> usize {
        self.path.len()
    }
}

impl<'a, 'n, Id, G> DoubleEndedIterator for PathIterator<'a, 'n, Id, G>
where
    Id: Copy + 'n,
    &'n G: IntoNeighborsDirected,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.path
            .next_back()
            .as_ref()
            .copied()
            .map(|n| &self.network[n])
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.path
            .nth_back(n)
            .as_ref()
            .copied()
            .map(|n| &self.network[n])
    }
}

impl<'n, Id, G> FusedIterator for PathIterator<'_, 'n, Id, G>
where
    Id: Copy + 'n,
    &'n G: IntoNeighborsDirected,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
{
}

#[must_use]
fn find_next_point(a: DVec3, fa: &Sphere, fb: &Sphere) -> DVec3 {
    let o = fa.intersection_midpoint(fb);
    let line = Line::new(a, o);
    let mb = line.sphere_intersection(fb).get_first_unchecked();
    line.interpolate(mb)
}

pub struct Segments<'a, 'n, Id, G>
where
    &'n G: GraphBase,
{
    network: &'n G,
    edges: Windows<'a, <&'n G as GraphBase>::NodeId>,
    a: DVec3,
    _id: PhantomData<[Id]>,
}

impl<'n, Id, G> Iterator for Segments<'_, 'n, Id, G>
where
    Id: Copy + 'n,
    &'n G: IntoNeighborsDirected,
    G: Index<<&'n G as GraphBase>::NodeId, Output = (Id, Sphere)>,
{
    type Item = (DVec3, Id);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let [a, b] = self.edges.next()? else {
            unreachable!("expected windows to be of length 2");
        };
        let (_, fa) = &self.network[*a];
        let (id, fb) = &self.network[*b];

        self.a = find_next_point(self.a, fa, fb);
        Some((self.a, *id))
    }
}
