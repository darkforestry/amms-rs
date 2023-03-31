
## Foreword
The recommended way to read this document is to fully read through the entire walkthrough once, without writing any code. This will allow you to grok the moving parts of the library as well as the architecture of the code without having to worry about AMM design before you know how everything fits together. The first read should be slow and steady, taking about 30 min. Then when you have reached the end of the walkthrough, you should pass through it again but adding code for the new AMM this time. The second pass through will allow you to fully focus on your AMM implementation instead of library architecture. As always, feel free to ask any questions in the DF discord if you run into any challenges.


## Adding a new AMM

`damms` was written with modularity in mind. The following is a straightforward walkthrough on how to add a new `AMM`. Note that this might seem complex at first, but this walkthrough is designed to make the integration process simple and easy. Just keep reading through the walkthrough, and you will have your new AMM integrated in no time! Once you are familiar with the codebase, building on top of the existing framework should be a breeze. Below is a quick overview of the steps to add a new AMM.

- Create a new module for your AMM
- Create a new AMM type
- Implement the `AutomatedMarketMaker` trait for the AMM
- Add your AMM to the `AMM` enum
- Add peripheral functions
- Add tests

Most AMMs will have a factory that is responsible for deploying the AMM. This factory allows `damms` to identify all of the AMMs from a given protocol, however some AMMs might not have a factory. If your AMM does have a factory, make sure to add a factory type with the following steps below.

- Create a factory type
- Implement the `AutomatedMarketMakerFactory` trait for the factory
- Add your factory to the `Factory` enum
- Add peripheral functions
- Add tests
- Add the factory to the `discovery` module

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
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>>;
    fn sync_on_event_signatures(&self) -> Vec<H256>;
    fn tokens(&self) -> Vec<H160>;
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError>;
    async fn populate_data<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>>;
}

```

Let's walk through what each function does. 
- `address`  simply returns the address for the given AMM. 
- `tokens` returns all of the tokens in the AMM as a `Vec<H160>`. For example, a `UniswapV2Pool` returns `[token_0, token_1]`. 
- `calculate_price` returns the price of `base_token` in the pool.
- `sync` gets any relevant AMM data at the most recent block. For example, the `sync` method for the `UniswapV2Pool` syncs `reserve0` and `reserve1`.
- `sync_on_event_signatures` returns all event signatures to subscribe to that will signal state changes in the AMM.
- `populate_data` fetches all of the peripheral AMM data (token addresses, token decimals, etc.) 


Once you have implemented the `AutomatedMarketMaker` trait, the next step is to add the new AMM to the `AMM` enum.

<br>

## Add your new AMM to the `AMM` enum

Now that your new AMM type is officially an `AutomatedMarketMaker`, we will add it to the `AMM` enum. Create a new AMM variant that wraps your newly created struct.

File: src/amm/mod.rs
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

    fn sync_on_event_signatures(&self) -> Vec<H256> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync_on_event_signatures(),
            AMM::UniswapV3Pool(pool) => pool.sync_on_event_signatures(),
            AMM::YourNewAMM(your_new_amm) => your_new_amm.sync_on_event_signatures(),
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

    async fn populate_data<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.populate_data(middleware).await,
            AMM::UniswapV3Pool(pool) => pool.populate_data(middleware).await,
            AMM::YourNewAMM(your_new_amm) => your_new_amm.populate_data(middleware).await,

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


`File: src/sync/checkpoints.rs`
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



In the same file, you will need to add your `AMM` to the match statement within `batch_sync_amms_from_checkpoint`. This function syncs all of the amms from a given factory. If your AMM has a factory, just add a `todo!()` and we will come back to this once the factory type has been implemented.

`File: src/sync/checkpoints.rs`
```rust
pub async fn batch_sync_amms_from_checkpoint<M: 'static + Middleware>(
    mut amms: Vec<AMM>,
    middleware: Arc<M>,
) -> JoinHandle<Result<Vec<AMM>, DAMMError<M>>> {
    let factory = match amms[0] {
        AMM::UniswapV2Pool(_) => Some(Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::zero(),
            0,
            0,
        ))),

        AMM::UniswapV3Pool(_) => Some(Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::zero(),
            0,
        ))),

        //Add a todo!() here, we will come back to this after we have implemented the AMM factory if applicable
        AMM::YourNewAMM(_) => todo!(),
    };

    //--snip--

}
```

In the case that the AMM you are adding does not have a factory, you can just add the following line instead of the snippet above.


`File: src/sync/checkpoints.rs`
```rust
pub async fn batch_sync_amms_from_checkpoint<M: 'static + Middleware>(
    mut amms: Vec<AMM>,
    middleware: Arc<M>,
) -> JoinHandle<Result<Vec<AMM>, DAMMError<M>>> {
    let factory = match amms[0] {
        AMM::UniswapV2Pool(_) => Some(Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::zero(),
            0,
            0,
        ))),

        AMM::UniswapV3Pool(_) => Some(Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::zero(),
            0,
        ))),

        //If there is not a factory for your AMM, you can just assign `factory` to None
        AMM::YourNewAMM(_) => None,
    };

    //--snip--
}
```

The last stop on our tour is the `populate_amms` function in `src/sync/mod.rs`. This function is responsible for getting all of the relevant `AMM` data for a given AMM. The content of the `amms` slice must contain the same `AMM` variant. There are two approaches to fetching the data. You can either create a [batch contract]() to get data for each of the AMMs in the vec in chunks or populate the data one by one. For the example below, we will implement data population without a batch contract.

```rust

pub async fn populate_amms<M: Middleware>(
    amms: &mut [AMM],
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    if amms_are_congruent(amms) {
        match amms[0] {
            AMM::UniswapV2Pool(_) => {
                let step = 127; //Max batch size for call
                for amm_chunk in amms.chunks_mut(step) {
                    uniswap_v3::batch_request::get_amm_data_batch_request(
                        amm_chunk,
                        middleware.clone(),
                    )
                    .await?;
                }
            }

            AMM::UniswapV3Pool(_) => {
                let step = 76; //Max batch size for call
                for amm_chunk in amms.chunks_mut(step) {
                    uniswap_v3::batch_request::get_amm_data_batch_request(
                        amm_chunk,
                        middleware.clone(),
                    )
                    .await?;
                }
            }

            //Populate data for each amm
            AMM::YourNewAMM(_)=>{
                for amm in amms {
                    amm.populate_data(middleware.clone()).await?;
                }
            }
        }
    } else {
        return Err(DAMMError::IncongruentAMMs);
    }

    //For each pair in the pairs vec, get the pool data
    Ok(())
}
```

<br>


## Add peripheral functions

Now that your new AMM is integrated into the `AMM` enum, its time to add peripheral functions. These are functions that are generally useful and specific to your AMM. These functions are not included in the `AutomatedMarketMaker` trait definition since different AMMs can have very specific internals, making it overly complex/inefficient to write a generic interface that encompasses all AMM variants now and in the future. While you won't get a compiler error if you do not integrate these functions, the following functions are necessary to have full functionality for swap routing and transaction creation.

- `pub fn new(args) -> YourNewAMMStruct`: Associated function to generate a new instance of the struct.

- `pub fn new_from_address(args) -> YourNewAMMStruct`: Associated function that generates a new populated struct with all of the relevant AMM data (see `UniswapV2Pool::new_from_address()` or `UniwsapV3Pool::new_from_address()` for reference).

- `pub fn simulate_swap(&self, token_in: H160, amount_in: U256) -> U256`: This function enables swap simulation which is critical for routing. Since the function does not have to adhere to a specific interface, you can add additional arguments like `token_out` or similar that relate specifically to your AMM. An `amount_out` represented as a `U256` should always be returned.

- `pub fn simulate_swap_mut(&self, token_in: H160, amount_in: U256) -> U256`: This function should be identical to the `simulate_swap` function with the difference being that the AMM should be mutated from the resulting swap. For example, on a UniswapV2 pool, `simulate_swap` simply returns the amount out, while `simulate_swap_mut` returns the amount_out and mutates the reserves based on the amount in.

- `pub fn fee(&self) -> u32`: If there is a fee associated with the AMM, it should be returned with this method.

- `pub fn swap_calldata(&self, args) -> Bytes`: This function takes in all of the arguments necessary for swapping tokens and returns the calldata that could be passed into a transaction or multicall.

- `pub fn sync_from_log(&self, log: &Log) -> Result<(), DAMMError<M>>`: Handles any logs and syncs the AMM accordingly. It is possible that an AMM needs to listen for multiple logs. If this is the case, this function should have pattern matching for each event signature and handle the log accordingly. This function should return an error if the log passed in does not match any signatures related to the AMM.

In addition to the functions above, feel free to write any other functions that might be useful like helper functions, calculations, etc.


<br>

## Add tests

Last but not least, make sure to add tests for all of the new functions introduced by your AMM. Quality tests will save long nights of debugging for a needle in a haystack. The goal is to make sure that we have tests for all functions to ensure confidence and stability in production mev systems.

<br>


## Adding a new AMM factory

Many AMMs will have a factory that is responsible for deploying / managing automated market makers associated with a protocol. Some AMMs might not have a factor however, it is very typical for DEXs and other CFMMs to have one. In the case that your AMM has a factory, we will add it to `damms` to enable identification of all AMMs related to that factory. An example of this is the UniswapV2Factory. This contract deploys all of the UniswapV2 pools and through the factory, we can identify every pool within the UniswapV2 ecosystem. In the following sections, we will walk through creating a factory type,k implementing the `AutomatedMarketMakerFactory` trait, adding the factory to the `Factory` enum, adding peripheral functions, adding tests and lastly adding the factory to the discovery module. This process is very similar to creating a new AMM by design. Let's get started.


<br>

## Create a new factory type

Let's head back into the `src/amm/your_new_amm/` directory that you created earlier and create a new file called `factory.rs`. Next, head into `src/amm/your_new_amm/mod.rs` and expose the new factory mod as public at the top of the file.

`File: src/amm/your_new_amm/mod.rs`
```rust
pub mod factory;
```


Now head back into `your_new_amm/factory.rs` and create a new struct to represent your factory. The factory must have at least an `address` and `creation_block` attribute. Make sure to also implement `#[derive(Clone, Copy, Serialize, Deserialize)]` as traits, these will come in handy later as well. Here is an example of what the `UniswapV2Factory` looks like.

`File: src/amm/uniswap_v2/factory.rs`
```rust

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct UniswapV2Factory {
    pub address: H160,
    pub creation_block: u64,
    pub fee: u32,
}
```

Now that we have a brand new factory type, lets move to the next step. 

<br>

## Implement the `AutomatedMarketMakerFactory` trait

Now we will need to implement the `AutomatedMarketMakerFactory` trait for the newly created factory struct. Let's take a look at the trait.

`File: src/amm/factory.rs`
```rust
#[async_trait]
pub trait AutomatedMarketMakerFactory {
    fn address(&self) -> H160;
    fn amm_created_event_signature(&self) -> H256;
    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error>;
    fn creation_block(&self) -> u64;

   async fn new_amm_from_log<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, DAMMError<M>>;

    async fn get_all_amms<M: Middleware>(
        &self,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>>;

    async fn populate_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>>;
}
```

Let's walk through what each function does. 
- `address` simply returns the address for the given AMM factory. 
- `amm_created_event_signature` returns the event signature that signals when an AMM is created from the factory.
- `new_empty_amm_from_log` creates a new empty AMM that coincides with the factory.
- `new_amm_from_log` creates a new AMM that coincides with the factory.
- `get_all_amms` gets all of the amms associated with that factory. For example the `UniswapV2Factory` gets all of the UniswapV2 pools.
- `populate_amm_data` populates all AMM data for the AMMs that are passed in. The AMMs must be related to the factory.
- `creation_block` is the block that the factory was created.



Once you have implemented the `AutomatedMarketMakerFactory` trait, the next step is to add the new factory to the `Factory` enum.


<br>


## Add your new factory to the `Factory` enum


Similarly to adding a new AMM to the `AMM` enum, you can add a new factory to the `Factory` enum by creating a new `Factory` variant and using your new factory type as the inner value as seen below.

`File: src/amm/factory.rs`
```rust
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Factory {
    UniswapV2Factory(UniswapV2Factory),
    UniswapV3Factory(UniswapV3Factory),
    YourNewFactory(YourNewFactoryStruct),
}
```

I'm sure you noticed, that change introduced a few compiler errors. The good news is that everything is contained in one spot within `src/amm/factory.rs`. You will need to add the new `Factory` variant is in the `AutomatedMarketMakerFactory` implementation for the `Factory` enum. Drop the new factory variant into each of the functions as seen below.

`File: src/amm/factory.rs`
```rust
#[async_trait]
impl AutomatedMarketMakerFactory for Factory {
    fn address(&self) -> H160 {
        match self {
            Factory::UniswapV2Factory(factory) => factory.address(),
            Factory::UniswapV3Factory(factory) => factory.address(),
            Factory::YourNewFactory(factory) => factory.address(),
        }
    }

    fn amm_created_event_signature(&self) -> H256 {
        match self {
            Factory::UniswapV2Factory(factory) => factory.amm_created_event_signature(),
            Factory::UniswapV3Factory(factory) => factory.amm_created_event_signature(),
            Factory::YourNewFactory(factory) => factory.amm_created_event_signature(),

        }
    }

    async fn new_amm_from_log<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, DAMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.new_amm_from_log(log, middleware).await,
            Factory::UniswapV3Factory(factory) => factory.new_amm_from_log(log, middleware).await,
            Factory::YourNewFactory(factory) => factory.new_amm_from_log(log, middleware).await,

        }
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.new_empty_amm_from_log(log),
            Factory::UniswapV3Factory(factory) => factory.new_empty_amm_from_log(log),
            Factory::YourNewFactory(factory) => factory.new_empty_amm_from_log(log),
        }
    }

    async fn get_all_amms<M: Middleware>(
        &self,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.get_all_amms(middleware).await,
            Factory::UniswapV3Factory(factory) => factory.get_all_amms(middleware).await,
            Factory::YourNewFactory(factory) => factory.get_all_amms(middleware).await,
        }
    }

    async fn populate_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.populate_amm_data(amms, middleware).await,
            Factory::UniswapV3Factory(factory) => factory.populate_amm_data(amms, middleware).await,
            Factory::YourNewFactory(factory) => factory.populate_amm_data(amms, middleware).await,
        }
    }

    fn creation_block(&self) -> u64 {
        match self {
            Factory::UniswapV2Factory(uniswap_v2_factory) => uniswap_v2_factory.creation_block,
            Factory::UniswapV3Factory(uniswap_v3_factory) => uniswap_v3_factory.creation_block,
            Factory::YourNewFactory(factory) => factory.creation_block,

        }
    }
}
```

<br>

## Add peripheral functions

Add any peripheral functions that might be helpful for the factory. Feel free to checkout the `UniswapV2Factory` in `src/amm/uniswap_v2/factory.rs` or `UniswapV3Factory` in `src/amm/uniswap_v3_factory.rs` for inspiration.

<br>

## Add tests

Just like with the AMM integration, make sure to add tests for all of the new functions introduced by your factory. It was said earlier but worth saying again, quality tests will save long nights of debugging for a needle in a haystack. The goal is to make sure that we have tests for all functions to ensure confidence and stability in production mev systems.


## Add the factory to `discovery`

The discovery mod uses AMM factories to discover protocols that adhere to the factory's interface. A walkthrough on how to add a factory to the `discovery` module is coming soon.
