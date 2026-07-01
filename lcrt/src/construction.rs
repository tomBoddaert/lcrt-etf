use std::net::Ipv4Addr;

use common::geo::Sphere;
use graph::Graph;
use petgraph::matrix_graph::{MatrixGraph, NodeIndex};
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::trace;

#[derive(Clone, Copy, Debug)]
struct Node<Id = Ipv4Addr> {
    id: Id,
    sphere: Sphere,
}

impl<Id> Node<Id> {
    fn mutual_coverage(a: &Self, b: &Self) -> bool {
        a.sphere.contains(b.sphere.o)
    }

    fn pair_edge_type(a: &Self, b: &Self) -> Option<Edge> {
        a.sphere.intersections(&b.sphere).map(|(ab, ba)| {
            if ab && ba {
                Edge::Connection
            } else {
                Edge::Intersection
            }
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct ConstructionNode<Id = Ipv4Addr> {
    node: Node<Id>,
    hop_distance: u16,
    availability: f32,
    interfering_neighbours: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Edge {
    Intersection,
    Connection,
    Link,
}

struct SystemConstruction<Id = Ipv4Addr, TraceHook = trace::Disabled> {
    connectivity: Graph<ConstructionNode<Id>, ()>,
    trace_hook: TraceHook,
}

#[derive(Debug)]
enum Trace<'a, Id = Ipv4Addr> {
    Level {
        level: u16,
        nodes: &'a [ConstructionNode<Id>],
        potential_forwarders: &'a FxHashSet<usize>,
        uncovered: &'a FxHashSet<usize>,
    },
    AddForwarder {
        nodes: &'a [ConstructionNode<Id>],
        forwarder: usize,
        children: &'a [usize],
    },
}

impl<Id> SystemConstruction<Id>
where
    Id: Copy,
{
    #[inline]
    const fn new() -> Self {
        Self::new_with_tracing(trace::Disabled)
    }
}

impl<Id, TraceHook> SystemConstruction<Id, TraceHook>
where
    Id: Copy,
    for<'t> TraceHook: trace::Hook<Trace<'t, Id>>,
{
    #[inline]
    const fn new_with_tracing(trace_hook: TraceHook) -> Self {
        Self {
            connectivity: Graph::new(),
            trace_hook,
        }
    }

    fn add(&mut self, node: ConstructionNode<Id>) {
        let i = self.connectivity.add_node(node);

        let parent_hop_distance = node.hop_distance.checked_sub(1);
        let child_hop_distance = node.hop_distance.checked_add(1);

        for j in self.connectivity.node_indices() {
            #[derive(Clone, Copy, Debug)]
            enum CandidateType {
                Parent,
                Child,
            }

            let candidate = &self.connectivity.node(j);
            let r#type = if Some(candidate.hop_distance) == parent_hop_distance {
                CandidateType::Parent
            } else if Some(candidate.hop_distance) == child_hop_distance {
                CandidateType::Child
            } else {
                continue;
            };

            if !Node::<Id>::mutual_coverage(&node.node, &candidate.node) {
                continue;
            }

            match r#type {
                CandidateType::Parent => self.connectivity.add_edge(j, i, ()),
                CandidateType::Child => self.connectivity.add_edge(i, j, ()),
            }
        }
    }

    fn construct(self) -> Graph<Node<Id>, Edge> {
        let Self {
            connectivity,
            mut trace_hook,
        } = self;

        let (nodes, mut connectivity) = connectivity.take_nodes();
        let mut network = Graph::with_nodes(nodes.iter().map(|cn| cn.node).collect());

        let levels = nodes
            .iter()
            .map(|n| n.hop_distance)
            .max()
            .unwrap_or_default();

        let mut uncovered = FxHashSet::default();
        let mut potential_forwarders = FxHashSet::default();
        let mut children = Vec::new();

        for l in (0..levels).rev() {
            extract_level(nodes.iter().enumerate(), l + 1, &mut uncovered);
            extract_level(nodes.iter().enumerate(), l, &mut potential_forwarders);

            trace_hook.trace(Trace::Level {
                level: l,
                nodes: &nodes,
                potential_forwarders: &potential_forwarders,
                uncovered: &uncovered,
            });

            while !uncovered.is_empty() {
                // TODO: can this be removed? handle None in the next part?
                // remove forwarders with no coverage
                potential_forwarders.retain(|&a| connectivity.neighbours(a).next().is_some());

                // find forwarder with highest eta
                let Some((forwarder, _)) = potential_forwarders
                    .iter()
                    .copied()
                    .map(|a| {
                        let children = connectivity.neighbours(a).count();
                        let eta = nodes[a].eta(children.try_into().unwrap_or(u16::MAX), 0); // TODO: update interfering nodes by keeping track of connectivity
                        (a, eta)
                    })
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                else {
                    // TODO: warn of abandoned nodes
                    // TODO: attempt to find new parents for the children in the abandoned subtree?
                    // for abandoned in uncovered.drain() {
                    //     delete_tree(&mut network, abandoned);
                    // }
                    todo!();
                    continue;
                };
                // remove the chosen forwarder from the potential forwarders
                potential_forwarders.remove(&forwarder);

                children.extend(connectivity.neighbours(forwarder));
                trace_hook.trace(Trace::AddForwarder {
                    nodes: &nodes,
                    forwarder,
                    children: &children,
                });
                for child in children.iter().copied() {
                    uncovered.remove(&child);
                    connectivity.disconnect_incoming(child);
                    network.add_edge(forwarder, child, Edge::Link);
                }
                children.clear();
            }
        }

        network
    }

    fn add_intersections(network: &mut MatrixGraph<Node, Edge, FxBuildHasher>) {
        // TODO: iterate through pairs, adding edges between ones with intersecting coverages
        // as nodes have been removed, the indexes do not follow a logical order
        todo!()
    }
}

fn extract_level<'i, I, Id>(nodes: I, level: u16, to: &mut FxHashSet<usize>)
where
    I: Iterator<Item = (usize, &'i ConstructionNode<Id>)>,
    Id: 'i,
{
    to.clear();
    to.extend(
        nodes
            .filter(|(_, n)| n.hop_distance == level)
            .map(|(i, _)| i),
    );
}

impl<Id> ConstructionNode<Id> {
    fn eta(&self, children: u16, additional_interfering_nodes: u16) -> f32 {
        crate::eta(
            self.availability,
            children,
            f32::from(self.interfering_neighbours) + f32::from(additional_interfering_nodes),
        )
    }
}

fn delete_tree<N, E>(graph: &mut MatrixGraph<N, E, FxBuildHasher>, root: NodeIndex) {
    while let Some(neighbour) = graph.neighbors(root).next() {
        delete_tree(graph, neighbour);
    }

    graph.remove_node(root);
}

#[cfg(test)]
mod test {
    use rustc_hash::FxHashSet;

    use crate::{
        construction::{ConstructionNode, SystemConstruction},
        test::tree_example,
        trace,
    };

    use super::Node;

    #[test]
    fn tree_connectivity() {
        let mut construction = SystemConstruction::new();

        for tree_example::NodeInfo {
            id,
            sphere,
            hop_distance,
            ..
        } in tree_example::NODES
        {
            construction.add(ConstructionNode {
                node: Node {
                    id: *id,
                    sphere: *sphere,
                },
                hop_distance: *hop_distance,
                availability: f32::INFINITY,
                interfering_neighbours: 0,
            });
        }

        let mut potential_children = FxHashSet::default();
        for i in construction.connectivity.node_indices() {
            let node = &tree_example::NODES[i];
            potential_children.extend(node.potential_children.iter().copied());
            for n in construction.connectivity.neighbours(i) {
                let n_id = tree_example::NODES[n].id;
                assert!(potential_children.remove(n_id));
            }
            assert!(
                potential_children.is_empty(),
                "{} missing connections to {potential_children:?}",
                node.id,
            );
        }
    }

    #[test]
    fn tree_construction() {
        struct State {
            level: u16,
            remaining_forwarders: FxHashSet<&'static str>,
        }
        impl trace::Hook<super::Trace<'_, &'static str>> for State {
            fn trace(&mut self, item: super::Trace<&'static str>) {
                match item {
                    super::Trace::Level { level, .. } => {
                        self.level = level;
                    }

                    super::Trace::AddForwarder {
                        nodes,
                        forwarder: f,
                        children,
                    } => {
                        let forwarder = &tree_example::NODES[f];
                        assert_eq!(nodes[f].node.id, forwarder.id);
                        assert!(self.remaining_forwarders.remove(&forwarder.id));

                        assert_eq!(forwarder.children.len(), children.len());
                        for &i in children {
                            assert!(forwarder.children.contains(&tree_example::NODES[i].id));
                        }
                    }
                }
            }
        }
        let mut state = State {
            level: u16::MAX,
            remaining_forwarders: tree_example::FORWARDERS.iter().copied().collect(),
        };

        let mut construction = SystemConstruction::<&'static str, _>::new_with_tracing(&mut state);

        for tree_example::NodeInfo {
            id,
            sphere,
            hop_distance,
            ..
        } in tree_example::NODES
        {
            construction.add(ConstructionNode {
                node: Node {
                    id: *id,
                    sphere: *sphere,
                },
                hop_distance: *hop_distance,
                availability: f32::INFINITY,
                interfering_neighbours: 0,
            });
        }

        let graph = construction.construct();

        assert_eq!(state.level, 0);
        assert!(state.remaining_forwarders.is_empty());

        let mut children = FxHashSet::default();
        for n in graph.node_indices() {
            let node = &tree_example::NODES[n];
            assert_eq!(graph.node(n).id, node.id);

            children.extend(node.children.iter().copied());
            for c in graph.neighbours(n) {
                assert!(children.remove(graph.node(c).id));
            }
            assert!(children.is_empty());
        }
    }
}
