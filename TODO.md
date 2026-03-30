## Common
- (b483) Create a type to manage node data, the network graph, and the intersection graph together in LCRT to be used by ETF.
- (5c6e) Add logging.
- (bce4) Resolve `todo!()`s and `// TODO:`s.

## LCRT
- (c711) Move `NodeInfo` input to `handle_` functions to allow passing borrowed state.

## ETF
- (7866: b483) Rewrite the SLT algorithm to use the intersection graph.
  - Take a starting forwarder node
