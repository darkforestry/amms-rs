# Introduction

`amms-rs` is a Rust library for interacting with automated market makers (AMMs) across EVM chains. This library provides logic to discover, sync and simulate swaps across a variety of pools. `amms-rs` also provides primitives to compose a state space of many pools without of the box logic to sync, filter and listen to state changes enabling you to focus on routing and strategy development instead.

<br>

`amms-rs` is structured around three core components:
- Pools
- Factories
- State Space


**Factories** are types used for discovering pools associated with a specific protocol.

**Pools** are protocol specific primitives for simulating swaps, computing prices and accessing pool state/metadata.

A **State Space** aggregates pools from multiple protocols. It handles syncing, filtering, and listening to state changes for pools in the state space. Additionally, the state space is able to simulate swaps over `n` pools to identify optimal routes.

<br>

`amms-rs` is designed to be modular/flexible and can be used in any context where interacting with AMMs is important. Some examples include MEV, swap aggregators/routers, DeFi protocols, price feeds, analytics backends, etc.



### Supported Protocols
Currently `amms-rs` supports the following protocols. If there is a specific AMM you'd like to see added, feel free to open an issue.

| AMM             | Status |
| --------------- | ------ |
| UniswapV2 | ✅     |
| UniswapV3 | ✅     |
| Balancer  | ✅     |
| ERC4626 Vaults | ✅     |
