# ETF
This library implements ETF independent of any specific platform. For more detail on the wider project, see the [root README](../README.md). Documentation is provided; see the [documentation section](../README.md#documentation).

## Key Parts
### Geometric Operations
The `geo` module defines geometric operations and types, particularly for our optimised long-distance straight-line trajectory checking implementation.

### Intersections
The `Intersections` type constructs an intersection graph over the node coverages. Eventually, this will be computed and updated by LCRT for efficiency. Its `get_path` method runs A\* over the graph to find a path through the network.

### Ancestor Path
The `get_ancestor_path` function attempts to find a path up the LCRT network tree to a forwarder that covers the target point.

### Straight Line Trajectory
The `get_straight_trajectory` function checks whether the given line is covered by the LCRT network using our optimised method. It currently does not return a `Path` but rather a vector that contains the same information.

### Path
The `path` module contains the `Path` result from some of the functions and iterators over the path points or segments. Each point is associated with a forwarder's address to switch to. LCRT's `change_parent` method should be used to request this switch.

## Examples
There is an example that constructs some paths in a tree network in [`examples/tree.rs`](examples/tree.rs).

## Integrating into Platforms
1. LCRT must be integrated.
2. When a transition is requested, given a target coordinate, the following steps must be followed:
  1. obtain the node's current position, then
  2. get the network graph from LCRT using the `get_network` method.
  3. With this, check for the existence of a covered straight line trajectory using `get_straight_trajectory`. If one exists, choose it; if not, then
  4. attempt to find a path in the network graph using `get_ancestor_path`. Again, if one is found, use it; otherwise,
  5. construct an intersection graph using `Intersections::new` and run A\* on it to find a path by running `Intersections::get_path`.
  This is all implemented as `etf_find_path` in the C API, although this is likely to change in future.
3. When following a path, the waypoints must be traversed following a straight line. At each waypoint, ETF must request that the node's LCRT manager change parent in the network to the corresponding new parent. It must wait for a matching parent changed signal before continuing.

## License
These libraries are dual-licensed under either the [MIT license](../LICENSE_MIT) or the [Apache license version 2.0](../LICENSE_Apache-2.0) at your option.

Integration modules may be licensed under different, compatible licenses. For our integration modules, see their respective repositories.
