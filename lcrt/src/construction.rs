use glam::DVec3;
use graph::Graph;
use petgraph::{
    matrix_graph::{MatrixGraph, NodeIndex},
    visit::IntoNodeReferences,
};
use rustc_hash::{FxBuildHasher, FxHashSet};

#[derive(Clone, Copy, Debug)]
struct Node {
    position: DVec3,
    radius: f64,
}

impl Node {
    #[inline]
    fn radius2(&self) -> f64 {
        self.radius * self.radius
    }

    #[inline]
    fn distance2(a: &Self, b: &Self) -> f64 {
        a.position.distance_squared(b.position)
    }

    fn mutual_coverage(a: &Self, b: &Self) -> bool {
        let d2 = Node::distance2(a, b);
        a.radius2() >= d2 && b.radius2() >= d2
    }

    fn pair_edge_type(a: &Self, b: &Self) -> Option<Edge> {
        let d2 = Node::distance2(a, b);
        let ra2 = a.radius2();
        let rb2 = b.radius2();

        // ra^2 + 2 ra rb + rb^2
        let sum_radius2 = (a.radius * b.radius).mul_add(2., ra2 + rb2);
        if sum_radius2 < d2 {
            return None;
        }

        Some(if ra2 >= d2 && rb2 >= d2 {
            Edge::Connection
        } else {
            Edge::Intersection
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct ConstructionNode {
    node: Node,
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

struct SystemConstruction {
    // network: MatrixGraph<ConstructionNode, Edge>,
    // connectivity: MatrixGraph<ConstructionNode, (), FxBuildHasher>,
    connectivity: Graph<ConstructionNode, ()>,
}

impl SystemConstruction {
    fn new() -> Self {
        Self {
            // connectivity: MatrixGraph::new(),
            connectivity: Graph::new(),
        }
    }

    fn add(&mut self, node: ConstructionNode) {
        let i = self.connectivity.add_node(node);

        let parent_hop_distance = node.hop_distance.checked_sub(1);
        let child_hop_distance = node.hop_distance.checked_add(1);

        // unless nodes are removed, their indexes are 0..len
        // this is used because <MatrixGraph as IntoNodeIdentifiers>::node_identifiers borrows from the graph
        // for j in 0..self.connectivity.node_count() {
        for j in self.connectivity.node_indices() {
            #[derive(Clone, Copy, Debug)]
            enum CandidateType {
                Parent,
                Child,
            }
            // let j = NodeIndex::new(j);

            // let candidate = &self.connectivity[j];
            let candidate = &self.connectivity.node(j);
            let r#type = if Some(candidate.hop_distance) == parent_hop_distance {
                CandidateType::Parent
            } else if Some(candidate.hop_distance) == child_hop_distance {
                CandidateType::Child
            } else {
                continue;
            };

            if !Node::mutual_coverage(&node.node, &candidate.node) {
                continue;
            }

            match r#type {
                CandidateType::Parent => self.connectivity.add_edge(j, i, ()),
                CandidateType::Child => self.connectivity.add_edge(i, j, ()),
            };
        }

        // for j in self.connectivity.node_identifiers() {
        //     if j == i {
        //         continue;
        //     }
        //     let b = &self.connectivity[j];

        //     if Node::mutual_coverage(&a, &self.connectivity[j].node) {
        //         self.connectivity.add_edge(i, j, ());
        //     }

        //     // let Some(edge) = Node::pair_edge_type(&info, &self.network[j].node) else {
        //     //     continue;
        //     // };

        //     // self.network.add_edge(i, j, edge);
        //     // self.network.add_edge(j, i, edge);
        // }
    }

    // fn construct(self) -> MatrixGraph<Node, Edge, FxBuildHasher> {
    fn construct(self) -> Graph<Node, Edge> {
        let Self { mut connectivity } = self;
        // let mut network = MatrixGraph::<Node, Edge, FxBuildHasher>::with_capacity_and_hasher(
        //     connectivity.node_count(),
        //     FxBuildHasher,
        // );
        // connectivity.node_references().for_each(|(i, a)| {
        //     let j = network.add_node(a.node);
        //     debug_assert_eq!(i, j);
        // });
        let (nodes, mut connectivity) = connectivity.take_nodes();
        let mut network = Graph::with_nodes(nodes.iter().map(|cn| cn.node).collect());

        // let levels = connectivity
        //     .node_references()
        let levels = nodes
            .iter()
            // .map(|(_, n)| n.hop_distance)
            .map(|n| n.hop_distance)
            .max()
            .unwrap_or_default();

        let mut uncovered = FxHashSet::default();
        let mut potential_forwarders = FxHashSet::default();
        let mut neighbours = Vec::new();

        for l in (0..levels).rev() {
            extract_level(
                // connectivity.node_references(),
                nodes.iter().enumerate(),
                l + 1,
                &mut uncovered,
            );
            extract_level(
                // connectivity.node_references(),
                nodes.iter().enumerate(),
                l,
                &mut potential_forwarders,
            );

            while !uncovered.is_empty() {
                // TODO: can this be removed? handle None in the next part?
                // remove forwarders with no coverage
                // potential_forwarders.retain(|&a| connectivity.neighbors(a).next().is_some());
                potential_forwarders.retain(|&a| connectivity.neighbours(a).next().is_some());

                // find forwarder with highest eta
                let Some((forwarder, _)) = potential_forwarders
                    .iter()
                    .copied()
                    .map(|a| {
                        // let children = connectivity.neighbors(a).count();
                        let children = connectivity.neighbours(a).count();
                        // let eta = connectivity[a].eta(children.try_into().unwrap_or(u16::MAX), 0); // TODO: update interfering nodes by keeping track of connectivity
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

                // neighbours.extend(connectivity.neighbors(forwarder));
                neighbours.extend(connectivity.neighbours(forwarder));
                for child in neighbours.iter().copied() {
                    uncovered.remove(&child);
                    // connectivity.remove_node(child);
                    connectivity.disconnect_incoming(child);
                    network.add_edge(forwarder, child, Edge::Link);
                }
                neighbours.clear();
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

// fn extract_level<'i, I>(nodes: I, level: u16, to: &mut FxHashSet<NodeIndex<u16>>)
fn extract_level<'i, I>(nodes: I, level: u16, to: &mut FxHashSet<usize>)
where
    // I: Iterator<Item = (NodeIndex<u16>, &'i ConstructionNode)>,
    I: Iterator<Item = (usize, &'i ConstructionNode)>,
{
    to.clear();
    to.extend(
        nodes
            .filter(|(_, n)| n.hop_distance == level)
            .map(|(i, _)| i),
    );
}

// fn coverage_neighbours(nodes: MatrixGraph<ConstructionNode)

impl ConstructionNode {
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
