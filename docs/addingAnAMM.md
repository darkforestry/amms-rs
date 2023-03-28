
## Adding an AMM

`damms` was written with modularity in mind. The following is a straightforward walkthrough on how to add a new `AMM`. Once you are familiar with the codebase, building on top of the existing framework should be a breeze. Below is a quick overview of the process.

- Create a new `mod` for your AMM
- Create a new AMM type
- Implement the `AutomatedMarketMaker` trait for the AMM
- Add your AMM to the `AMM` enum
- Add peripheral functions
- Add tests

TODO: note on if factory is applicable, add it
- Create a factory type
- Implement the `AutomatedMarketMakerFactory` trait for the factory
- Add your factory to the `Factory` enum
- Add peripheral functions
- 


Adhere to the interface you will see where it breaks but also highlight where it breaks
Also add sim swap, sim swap mut, swap calldata, and list others