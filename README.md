# A Platform-Independent Implementation of LCRT & ETF
The Efficient Transition Formation algorithm (ETF)[\[1\]][1] constructs paths for Unmanned Aerial Vehicles (UAVs) in LCRT networks. These paths are created such that the UAV never leaves the network.

The Link-Controlled Routing Tree algorithm (LCRT)[\[2\]][2] is a multicast ad hoc routing protocol for efficient data streaming, for example video streaming.

To maximise the applicability of our implementation and its impact on the research community, we implement each algorithm in an abstracted, platform-independent manner. These libraries have been integrated into the [ns-3 simulator](https://www.nsnam.org/) and the [INET framework](https://inet.omnetpp.org/) for the [OMNeT++ simulator](https://omnetpp.org/).

## This Repository
In this repository, we provide our platform-independent implementations of LCRT and ETF, and a C-API to ease integration into C/C++-based platforms.

More information on each implementation can be found in the READMEs for [LCRT](lcrt/README.md), [ETF](etf/README.md), and [`lcrt_c`](lcrt_c/README.md).

### Requirements
Building requires an up-to-date installation of the [cargo package manager](https://doc.rust-lang.org/cargo/). This can be installed using [rustup](https://rustup.rs/).

### Documentation
Documentation can be built and opened with the following command.
```sh
cargo doc --no-deps --open
```

## Platform Integration Modules
These libraries have been integrated into ns-3 and INET in OMNeT++. These repositories can be found at [lcrt-etf-ns-3](https://github.com/tomBoddaert/lcrt-etf-ns-3) and [lcrt-etf-omnetpp](https://github.com/tomBoddaert/lcrt-etf-omnetpp).

### Integrating into More Platforms
See the [LCRT README](lcrt/README.md#integrating-into-platforms) and the [ETF README](etf/README.md#integrating-into-platforms) sections on integrating the algorithms into platforms.

## License
These libraries are dual-licensed under either the [MIT license](LICENSE_MIT) or the [Apache license version 2.0](LICENSE_Apache-2.0) at your option.

Integration modules may be licensed under different, compatible licenses. For our integration modules, see their respective repositories.

## References
### ETF
W. Tu, "Resource-efficient seamless transitions for high-performance multi-hop UAV multicasting," in Computer Networks, vol. 213, 2022, 109051, ISSN 1389–1286, <https://doi.org/10.1016/j.comnet.2022.109051>.

### LCRT
W. Tu, C. J. Sreenan, C. T. Chou, A. Misra and S. Jha, "Resource-Aware Video Multicasting via Access Gateways in Wireless Mesh Networks," in IEEE Transactions on Mobile Computing, vol. 11, no. 6, pp. 881–895, June 2012, <https://doi.org/10.1109/TMC.2011.103>.

[1]: #ETF
[2]: #LCRT
