
## Adding a new AMM

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

<br>

## Create a new module for your AMM

Welcome to the first and easiest step of the walkthrough. Currently all AMMs are located in the `src/amm`. Create a new directory with the name of your AMM in the `src/amm` directory and add a new `mod.rs` file within your newly created directory.

```
- src
    - amm
        - uniswap_v2
        - uniswap_v3
        - your_new_amm
            - mod.rs
```

Thats it for this step, great job and congrats (insert champagne popping gif here).


<br>

## Create a new AMM type
Now lets head to the newly created `mod.rs` file in the directory that you just initialized and write some code. Within this file, you will want to create a new struct for your AMM. Here is an example of what the `UniswapV2Pool` type looks like to get a rough idea of what the type should look like. Your new AMM will potentially look very different depending on the mechanics of the AMM itself.

File: src/amm/uniswap_v2/mod.rs
```rust
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct UniswapV2Pool {
    pub address: H160,
    pub token_a: H160,
    pub token_a_decimals: u8,
    pub token_b: H160,
    pub token_b_decimals: u8,
    pub reserve_0: u128,
    pub reserve_1: u128,
    pub fee: u32,
}
```

Make sure that you have an `address` field in your struct as this will come in handy later. Also, make sure to implement the traits defined above the `UniswapV2Pool` (`#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
`) on your struct as these will also be important later during syncing. Make sure that the struct is as lean as possible, but don't compromise simplicity. If it makes swap simulation much easier to store a specific attribute in your struct at the cost of size, it probably makes sense to do so. If you are unsure, feel free to get some feedback in the DF discord channel. 

Now that we have a newly created struct, lets head to the next section.

<br>


## Implement the `AutomatedMarketMaker` trait

Now we will need to implement the `AutomatedMarketMaker` on your newly created struct. Lets take a look at the trait.


File: src/amm/mod.rs
```rust
#[async_trait]
pub trait AutomatedMarketMaker {
    fn address(&self) -> H160;    
    fn tokens(&self) -> Vec<H160>;
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError>;
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>>;
    fn sync_on_event_signature(&self) -> H256;
}
```

Let's walk through what each function does. 
- `address`  simply returns the address for the given AMM. 
- `tokens` returns all of the tokens in the AMM as a `Vec<H160>`. For example, a `UniswapV2Pool` returns `[token_0, token_1]`. 
- `calculate_price` returns the price of `base_token` in the pool.
- `sync` gets any relevant AMM data at the most recent block. For example, the `sync` method for the `UniswapV2Pool` syncs `reserve0` and `reserve1`.
- `sync_on_event_signature` returns the event signature to subscribe to that will signal state changes in the AMM.


Once you have implemented the `AutomatedMarketMaker` trait, the next step is to add the new AMM to the `AMM` enum.

<br>

## Add the new AMM to the `AMM` enum

Now that your new AMM type is officially an `AutomatedMarketMaker`, we will add it to the `AMM` enum. Create a new AMM variant that wraps your newly created struct.

File: src/amm/mod.rs
```rust
#[async_trait]
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum AMM {
    UniswapV2Pool(UniswapV2Pool),
    UniswapV3Pool(UniswapV3Pool),
    YourNewAMM(YourNewAMMStruct)
}
```

And all of a sudden, red everywhere. You will notice that after adding your AMM variant, many things break. Fear not, this is a feature not a bug. `damms` uses exhaustive pattern matching for the `AMM` enum so that you know exactly where to add your new variant throughout the codebase. Lets take a look at each spot.

