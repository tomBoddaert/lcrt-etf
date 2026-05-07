# LCRT
This library implements LCRT independent of any specific platform. For more detail on the wider project, see the [root README](../README.md). Documentation is provided; see the [documentation section](../README.md#documentation).

## Key Parts
### Control Message Definitions
The `message` module defines each kind of control message used in LCRT.
- `AreaConstruction` advertises the construction of an LCRT network
- `AreaInfo` provides information about a constructed LCRT network and its topology
- `JoinAccept` accepts a join offer created by a `JoinAvailable` message
- `JoinArea` requests to join an LCRT network
- `JoinAvailable` advertises a join offer in response to a `JoinArea` message
- `JoinReport` requests to join an LCRT network during construction

`Message` is an enum of all the control messages.

### State Managers
We define two types to handle the state of a node in an LCRT network: `Area` and `AreaSource`. The latter is for the source. These take inputs in terms of messages and timers via their `handle_message` and `handle_timeout` methods.

The source performs the area construction algorithm, building the multicast tree.

### State Update Response
`handle_*` methods return a `Response`. This may contain:
- a control message to broadcast to neighbouring nodes
- a timeout to set
- an event to emit (to be listened to by algorithms such as ETF)

### Configuration
The `Config` struct defines constants needed for the LCRT algorithm. See its documentation for descriptions of each property.

### Node Interface
The `NodeInfo` trait defines methods for obtaining information about the network node and platform. This should be implemented by the platform integration module.

## Integrating into Platforms
1. If using the C API, clone this repository into the source tree and add a build step to the build system. If using the Rust API, then this can be added as a library using cargo's git option.
2. Create a boiler-plate UDP multicast routing algorithm in the given platform with two timers, indexable by keys `1` and `2`.
3. Create a header containing the IPv4 address of the previous forwarder and a packet ID (`u8`).
4. An implementation of `NodeInfo` should be written. If using the C API, this is encapsulated in the `LcrtNodeInfo` struct where the context may be an instance of the routing implementation.
5. Create an instance of `Area`, `AreaSource`, or `AreaAny` for each routing instance, passing in the node interface and a config, consistent across all nodes in a universe/simulation. If using the C API, the `LcrtArea` type should be used.
6. Control messages should be passed to the `handle_message` method on the area instance.
7. A function should be created to handle the `Result` type that is returned by `handle_*` methods. This may contain the following:
  - a control message that must be broadcast to neighbouring nodes,
  - a timer duration and index. If this is present, the matching timer must be reset with the new duration. And finally,
  - an event, which should be emitted using the platform's event or signal system. Any system that an ETF implementation would be able to listen to will work.
8. A header or trailer must be added to each routed packet; this will contain the previous forwarder's address and a packet ID obtained using the `next_packet_id` method.
9. When choosing whether to accept a packet, it must meet the following criteria:
  - it must have been sent to the group that the LCRT network manages, and
  - its previous forwarder must be the parent of the current node. This can be checked using the `get_parent` method. In the C API, the `is_parent` method should be used instead.
  If these are met, then the `notify_received_packet` method should be called and the packet accepted.
10. Additionally, if the node is a forwarder, the packet should be forwarded. This can be checked with the `has_children` method or `lcrt_area_is_forwarder` in the C API.

## License
These libraries are dual-licensed under either the [MIT license](../LICENSE_MIT) or the [Apache license version 2.0](../LICENSE_Apache-2.0) at your option.

Integration modules may be licensed under different, compatible licenses. For our integration modules, see their respective repositories.
