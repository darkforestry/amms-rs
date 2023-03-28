
## Adding an AMM

`damms` was written with modularity in mind. The following is a straightforward walkthrough on how to add a new `AMM`. Once you are familiar with the codebase, building on top of the existing framework should be a breeze. Below is a quick overview of the process.

- Create a new module for your AMM
- Create a new AMM type
- Implement the `AutomatedMarketMaker` trait for the AMM
- Add your AMM to the `AMM` enum
- Add peripheral functions
- Add tests

Most AMMs will have a factory that is responsible for deploying the AMM. This factory allows `damms` to identify all of the AMMs from a given protocol, however some AMMs might not have a factory. In the case that the AMM you are adding does have a factory, make sure to add a factory type with the following steps below.

- Create a factory type
- Implement the `AutomatedMarketMakerFactory` trait for the factory
- Add your factory to the `Factory` enum
- Add peripheral functions


Adhere to the interface you will see where it breaks but also highlight where it breaks
Also add sim swap, sim swap mut, swap calldata, and list others

With the overview out of the way, lets start by creating a mod for your brand new AMM.


### Create a new module for your AMM

Welcome to the first and easiest step of the walkthrough. Currently all AMMs are located in the `src/amm`. Create a new directory with the name of your AMM in the `src/amm` directory and add a new `mod.rs` file within your newly created directory.

```
- src
    - amm
        - uniswap_v2
        - uniswap_v3
        - your_new_amm
            - mod.rs
```

