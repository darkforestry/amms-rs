# amms-rs [![Github Actions][gha-badge]][gha]
[gha]: https://github.com/darkforestry/amms-rs/actions
[gha-badge]: https://github.com/darkforestry/amms-rs/actions/workflows/ci.yml/badge.svg

`amms-rs` is a Rust library to interact with automated market makers across EVM chains. This lib provides functionality to [discover](https://github.com/darkforestry/amms-rs/blob/main/examples/discover-factories.rs), [sync](https://github.com/darkforestry/amms-rs/blob/main/examples/sync-amms.rs), [filter](https://github.com/darkforestry/amms-rs/blob/main/examples/filter-value.rs), and interact with a variety of AMMs. This library also provides functionality to keep a [state space synced](https://github.com/darkforestry/amms-rs/blob/main/examples/state-space.rs), abstracting logic to handle chain reorgs, maintaining a state change cache and more. `amms-rs` was built with modularity in mind, making it quick and easy to add a new `AMM` variant by implementing the `AutomatedMarketMaker` trait. For a full walkthrough on how to quickly implement a new `AMM`, check out [`addingAnAMM.md`](https://github.com/darkforestry/amms-rs/blob/main/docs/addingAnAMM.md).


## ------------------------------------
## Tests and Docs are still being written 🏗️.
Tests are still being written, assume bugs until tested. If you would like to help contribute on the tests or docs, feel free to open up an issue or make a PR.
## ------------------------------------





## Supported AMMs

| AMM | Status |
|----------|------|
| UniswapV2 Pools | ✅||
| UniswapV3 Pools | ✅||
| ERC4626 Vaults | ✅||
| Izumi Pools | 🟨||
| Curve Pools | ❌||
| Balancer Pools | ❌||
| Bancor Pools | ❌||




