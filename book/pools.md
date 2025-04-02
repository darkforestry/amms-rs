# Pools

A pool in `amms-rs` is any type that implements the `AutomatedMarketMakerTrait`. This trait defines methods to sync an individual pool, calculate the price, simulate a swap and get generic metadata about the pool.


Filename: `src/amms/amm.rs`
```rust
pub trait AutomatedMarketMaker {
    /// Address of the AMM
    fn address(&self) -> Address;

    /// Event signatures that indicate when the AMM should be synced
    fn sync_events(&self) -> Vec<B256>;

    /// Syncs the AMM state
    fn sync(&mut self, log: &Log) -> Result<(), AMMError>;

    /// Returns a list of token addresses used in the AMM
    fn tokens(&self) -> Vec<Address>;

    /// Calculates the price of `base_token` in terms of `quote_token`
    fn calculate_price(&self, base_token: Address, quote_token: Address) -> Result<f64, AMMError>;

    /// Simulate a swap
    /// Returns the amount_out in `quote token` for a given `amount_in` of `base_token`
    fn simulate_swap(
        &self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError>;

    /// Simulate a swap, mutating the AMM state
    /// Returns the amount_out in `quote token` for a given `amount_in` of `base_token`
    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError>;

    // Initializes an empty pool and syncs state up to `block_number`
    async fn init<N, P>(self, block_number: BlockId, provider: P) -> Result<Self, AMMError>
    where
        Self: Sized,
        N: Network,
        P: Provider<N> + Clone;
}
```


## Initializing a new pool
All pools feature a `new()` method that constructs a new, "unsynced" pool. In this context, "unsynced" means that any data that onchain data associated with the pool is not populated. Lets take a quick look at the `UniswapV3Pool` as an example.

Filename: `src/amms/uniswap_v3/mod.rs`
```rust
pub struct UniswapV3Pool {
    pub address: Address,
    pub token_a: Token,
    pub token_b: Token,
    pub liquidity: u128,
    pub sqrt_price: U256,
    pub fee: u32,
    pub tick: i32,
    pub tick_spacing: i32, // TODO: we can make this a u8, tick spacing will never exceed 200
    pub tick_bitmap: HashMap<i16, U256>,
    pub ticks: HashMap<i32, Info>,
}

```

<br>

Upon Construction of the pool, the `address` field is populated, but all other fields are not.

```rust
// Initialize a new, unsynced pool
let pool = UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"));

// Log the pool
dbg!(pool);
```

```
pool = UniswapV3Pool {
    address: 0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640,
    token_a: Token {
        address: 0x0000000000000000000000000000000000000000,
        decimals: 0,
    },
    token_b: Token {
        address: 0x0000000000000000000000000000000000000000,
        decimals: 0,
    },
    liquidity: 0,
    sqrt_price: 0,
    fee: 0,
    tick: 0,
    tick_spacing: 0,
    tick_bitmap: {},
    tick
}
```

To sync onchain data, all AMMs feature a method to initialize the pool via the `AutomatedMarketMaker::init()` function. Taking a look at the same example, calling the `init()` method populates all other metadata associated wtih the pool.

```rust
// Initialize a new, pool and initialize
 let pool = UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"))
    .init(BlockId::latest(), provider)
    .await?;

// Log the pool
dbg!(pool);
```

```
pool = UniswapV3Pool {
    address: 0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640,
    token_a: Token {
        address: 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48,
        decimals: 6,
    },
    token_b: Token {
        address: 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2,
        decimals: 18,
    },
    liquidity: 10809957708954355118,
    sqrt_price: 1825965660595559818034121050310406,
    fee: 500,
    tick: 200915,
    tick_spacing: 10,

    // --snip--
}
```

Note that syncing individual pools is typically only useful for testing or very specific use cases. In most cases, you will want to sync multiple AMMs at once and keep pools up to date with the latest state changes. For this functionality, it is recommended to use the [StateSpaceBuilder](./state_space.md) to sync and maintain AMMs within a state space.


## Swap Simulation

All pools also feature a function to simulate swaps via the `AutomatedMarketMaker::simulate_swap` trait method. This function allows you to specify an `amount_in`, `base_token` (token in) and `quote_token` (token_out) and receive the corresponding `amount_out`. Note that for pools that only have two tokens, the `quote_token` does not need to be specified.

```rust
// Simulate swap with UniswapV3 Pool
let uv3_pool = UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"))
    .init(BlockId::latest(), provider)
    .await?;

// Note that the token out does not need to be specified 
// when simulating a swap for pools with only two tokens.
let amount_out = pool.simulate_swap(
    pool.token_a.address,
    Address::default(),
    U256::from(1000000),
)?;


// Simulate swap with Balancer Pool
let balancer_pool = BalancerPool::new(address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"))
    .init(BlockId::latest(), provider)
    .await?;

let tokens = balancer_pool.tokens();

// Since Balancer pools can have more than 2 tokens
// we must specify the token in and token out
let amount_out = balancer_pool.simulate_swap(
    tokens[0].address,
    tokens[1].address,
    U256::from(1000000),
)?;
```

TODO: add example showing how to simulate a route


TODO: add example showing how to generate swap calldata