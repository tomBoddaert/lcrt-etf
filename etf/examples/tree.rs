use etf::{
    geo::{Line, Sphere},
    get_ancestor_path, get_straight_trajectory,
};
use glam::DVec3;
use petgraph::{Directed, matrix_graph::MatrixGraph};
use rustc_hash::FxHashMap;

const NODES: &[(&str, Sphere)] = &[
    ("A", Sphere::with_components(70., 140., 35., 50.)),
    ("B", Sphere::with_components(40., 110., 20., 50.)),
    ("C", Sphere::with_components(110., 120., 20., 50.)),
    ("D", Sphere::with_components(0., 110., 0., 50.)),
    ("E", Sphere::with_components(20., 70., 20., 50.)),
    ("F", Sphere::with_components(120., 80., 40., 50.)),
    ("G", Sphere::with_components(10., 30., 30., 50.)),
    ("H", Sphere::with_components(120., 40., 20., 50.)),
    ("I", Sphere::with_components(100., 0., 30., 50.)),
];
const EDGES: &[(&str, &str)] = &[
    ("A", "B"),
    ("A", "C"),
    ("B", "D"),
    ("B", "E"),
    ("C", "F"),
    ("E", "G"),
    ("F", "H"),
    ("H", "I"),
];

type NodeMap = FxHashMap<&'static str, u16>;
type Network =
    MatrixGraph<(&'static str, Sphere), (), rustc_hash::FxBuildHasher, Directed, Option<()>, u16>;
type Intersections<'n> = etf::Intersections<'n, &'static str, Network>;
fn main() {
    let nodes: NodeMap = NODES
        .iter()
        .copied()
        .zip(0_u16..)
        .map(|((id, _), ix)| (id, ix))
        .collect();
    let mut network = Network::with_capacity(NODES.len());
    for (data, expected_ix) in NODES.iter().zip(0_u16..) {
        let ix = network.add_node(*data);
        assert_eq!(ix, expected_ix.into());
    }
    network.extend_with_edges(EDGES.iter().copied().map(|(a, b)| (nodes[a], nodes[b])));
    let intersections = Intersections::new(&network);

    straight_path_successful();
    println!();
    ancestor_path_successful(&nodes, &network);
    println!();
    intersection_path_successful(&intersections);
}

fn straight_path_successful() {
    let start_point = DVec3::new(0., 20., 50.);
    let target = DVec3::new(-45., 105., 0.);

    println!("Straight line trajectory: {start_point} -> {target}");

    let trajectory = Line::new(start_point, target);
    let path = get_straight_trajectory(trajectory, NODES.iter().copied())
        .expect("expected to accept the straight line path");

    for (id, _) in &path {
        print!("{id} -> ");
    }
    println!("target");

    for (id, point) in &path {
        print!("{point:.2} {id} -> ");
    }
    println!("{target}");
}

fn ancestor_path_successful(nodes: &NodeMap, network: &Network) {
    let start = "G";
    let start_point = DVec3::new(0., 20., 50.);
    let target = DVec3::new(50., 140., 30.);
    let expected = &["G", "E", "B"];

    println!("Ancestor path: {start_point} -> {target}");

    let path = get_ancestor_path(network, nodes[start].into(), target)
        .expect("expected to find a ancestor path");

    for (id, _) in &path {
        print!("{id} -> ");
    }
    println!("target");

    assert!(path.iter().map(|(id, _)| id).eq(expected));

    print!("{start_point}");
    for (point, id) in path.segments(start_point) {
        print!(" -> {point:.2} {id}");
    }
    println!(" -> {target}");
}

fn intersection_path_successful(intersections: &Intersections) {
    let start = "G";
    let start_point = DVec3::new(0., 20., 50.);
    let target = DVec3::new(100., 50., 0.);

    println!("Intersection path: {start_point} -> {target}");

    let path = intersections
        .get_path(intersections.get_ix(&start), target)
        .expect("expected to find a path");

    for (id, _) in &path {
        print!("{id} -> ");
    }
    println!("target");

    print!("{start_point}");
    for (point, id) in path.segments(start_point) {
        print!(" -> {point:.2} {id}");
    }
    println!(" -> {target}");
}
