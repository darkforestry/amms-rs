
## Foreword
The recommended way to read this document is to fully read through the entire walkthrough once, without writing any code. This will allow you to grok the moving parts of the library as well as the architecture of the code without having to worry about AMM design before you know how everything fits together. The first read should be slow and steady, taking about 30 - 60 min. Then when you have reached the end of the walkthrough, you should pass through it again but adding code for the new AMM this time. The second pass through will allow you to fully focus on your AMM implementation instead of library architecture. As always, feel free to ask any questions in the DF discord if you run into any challenges.


## Adding a new AMM

`damms` was written with modularity in mind. The following is a straightforward walkthrough on how to add a new `AMM`. Note that this might seem complex at first, but this walkthrough is designed to make the integration process simple and easy. Just keep reading through the walkthrough, and you will have your new AMM integrated in no time! Once you are familiar with the codebase, building on top of the existing framework should be a breeze. Below is a quick overview of the steps to add a new AMM.

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


Lastly, we will add the new AMM and factory to the discovery module (walkthrough coming soon).

With the overview out of the way, let's start by creating a mod for your brand new AMM.

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

After creating the `mod.rs` file, make sure to declare the module as public in `amm/mod.rs` at the top of the file.


`File: src/amm/mod.rs`
```rust
pub mod factory;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod your_new_amm;
```


Thats it for this step, great job and congrats (insert champagne popping gif here).


<br>

## Create a new AMM type
Now let's head to the newly created `mod.rs` file in the directory that you just initialized and write some code. Within this file, you will want to create a new struct for your AMM. Here is an example of what the `UniswapV2Pool` type looks like to get a rough idea of what the type should look like. Your new AMM will potentially look very different depending on the mechanics of the AMM itself.

`File: src/amm/uniswap_v2/mod.rs`
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

Now that we have a newly created struct, let's head to the next section.

<br>


## Implement the `AutomatedMarketMaker` trait

Now we will need to implement the `AutomatedMarketMaker` on your newly created struct. Let's take a look at the trait.


`File: src/amm/mod.rs`
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

## Add your new AMM to the `AMM` enum

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

And all of a sudden, red everywhere. You will notice that after adding your AMM variant, many things break. Fear not, this is a feature not a bug. `damms` uses exhaustive pattern matching for the `AMM` enum so that you know exactly where to add your new variant throughout the codebase. Let's take a look at each spot.



The first spot we need to add code is the `AutomatedMarketMaker` implementation for the `AMM` enum. We use an enum dispatch so that we can put all `AMM` variants in a collection and call any of the `AutomatedMarketMaker` methods on the `AMM` enum itself without having to match on the inner types.

`File: src/amm/mod.rs`
```rust

#[async_trait]
impl AutomatedMarketMaker for AMM {
    fn address(&self) -> H160 {
        match self {
            AMM::UniswapV2Pool(pool) => pool.address,
            AMM::UniswapV3Pool(pool) => pool.address,
            AMM::YourNewAMM(your_new_amm) => your_new_amm.address,
        }
    }

    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync(middleware).await,
            AMM::UniswapV3Pool(pool) => pool.sync(middleware).await,
            AMM::YourNewAMM(your_new_amm) => your_new_amm.sync(middleware).await,
        }
    }

    fn sync_on_event_signature(&self) -> H256 {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync_on_event_signature(),
            AMM::UniswapV3Pool(pool) => pool.sync_on_event_signature(),
            AMM::YourNewAMM(your_new_amm) => your_new_amm.sync_on_event_signature(),
        }
    }

    fn tokens(&self) -> Vec<H160> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.tokens(),
            AMM::UniswapV3Pool(pool) => pool.tokens(),
            AMM::YourNewAMM(your_new_amm) => your_new_amm.tokens(),
        }
    }

    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.calculate_price(base_token),
            AMM::UniswapV3Pool(pool) => pool.calculate_price(base_token),
            AMM::YourNewAMM(your_new_amm) => your_new_amm.calculate_price(base_token),
        }
    }
}
```


Next, let's head over to `src/sync/mod.rs`. The following function is responsible for removing AMMs that did not populate correctly from a given `Vec<AMM>`.


`File: src/sync/mod.rs`
```rust
pub fn remove_empty_amms(amms: Vec<AMM>) -> Vec<AMM> {
    let mut cleaned_amms = vec![];

    for amm in amms {
        match amm {
            AMM::UniswapV2Pool(uniswap_v2_pool) => {
                if !uniswap_v2_pool.token_a.is_zero() && !uniswap_v2_pool.token_b.is_zero() {
                    cleaned_amms.push(amm)
                }
            }

            AMM::UniswapV3Pool(uniswap_v3_pool) => {
                if !uniswap_v3_pool.token_a.is_zero() && !uniswap_v3_pool.token_b.is_zero() {
                    cleaned_amms.push(amm)
                }
            }

            AMM::YourNewAMM(your_new_amm) => {
                //This can be anything to signal that the pool has been populated
                if your_new_amm.some_condition() {
                    cleaned_amms.push(amm)
                }
            }
        }
    }

    cleaned_amms
}
```

Let's head to `src/sync/checkpoint.rs` for the next snippet. The `sort_amms` function is used during syncing and sorts the amms into separate `Vec`s so that syncing can happen via batch contracts (more on this later). We will add a few things to this function. First, add a new collection where all the AMMs that match your new variant will be sorted into. Then, add another `Vec<AMM>` to the return value. Then you can add your new collection to the return statement at the bottom of the function. Lastly, add pattern matching for your new `AMM` variant and push AMMs that match your variant to the new collection you just made. Below is an example of the completed function.

```rust

//Add another Vec<AMM> to the return value
pub fn sort_amms(amms: Vec<AMM>) -> (Vec<AMM>, Vec<AMM>, Vec<AMM>) {
    let mut uniswap_v2_pools = vec![];
    let mut uniswap_v3_pools = vec![];
    
    //Add a vec to collect the sorted AMMs that match your variant
    let mut your_new_amm_collection = vec![];

    for amm in amms {
        match amm {
            AMM::UniswapV2Pool(_) => uniswap_v2_pools.push(amm),
            AMM::UniswapV3Pool(_) => uniswap_v3_pools.push(amm),
            AMM::YourNewAMM(_) => your_new_amm_collection.push(amm),

        }
    }

    //Add the collection for your new variant to the return statement
    (uniswap_v2_pools, uniswap_v3_pools, your_new_amm_collection)
}
```

