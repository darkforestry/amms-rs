## Adding a new AMM

The `AutomatedMarketMaker` trait defines the methods required to interact with an AMM, including syncing, accessing pool data, and simulating operations. To add a new AMM:

- Add a new module under `amms/` where the directory is the name of the new AMM.
- Create a struct to represent the new AMM and implement the `AutomatedMarketMaker` trait.
- Add your AMM type to the `amm!()` macro, which will make the new pool type via the `AMM` enum.

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
    async fn init<T, N, P>(self, block_number: BlockId, provider: Arc<P>) -> Result<Self, AMMError>
    where
        Self: Sized,
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>;
}

// --snip--
// Add your AMM to the `amm!()` macro
amm!(UniswapV2Pool, UniswapV3Pool, Balancer, YourAMMType)
```



If your AMM has a factory:
- Create a new struct representing the AMM factory and implement the `AutomatedMarketMakerFactory` trait.
- Add your factory type to the `factory!()` macro, which will make the new pool type via the `Factory` enum.

```rust

pub trait AutomatedMarketMakerFactory: DiscoverySync {
    type PoolVariant: AutomatedMarketMaker + Default;

    /// Address of the factory contract
    fn address(&self) -> Address;

    /// Creates an unsynced pool from a creation log.
    fn create_pool(&self, log: Log) -> Result<AMM, AMMError>;

    /// Returns the block number at which the factory was created.
    fn creation_block(&self) -> u64;

    /// Event signature that indicates when a new pool was created
    fn pool_creation_event(&self) -> B256;

    /// Event signatures signifying when a pool created by the factory should be synced
    fn pool_events(&self) -> Vec<B256> {
        Self::PoolVariant::default().sync_events()
    }

    fn pool_variant(&self) -> Self::PoolVariant {
        Self::PoolVariant::default()
    }
}

// --snip--
factory!(UniswapV2Factory, UniswapV3Factory, BalancerFactory, YourFactoryType);
```

Now you can include the new factory type when discovering/syncing all pools via the `StateSpaceBuilder`.

```rust
 let factories = vec![
        // UniswapV2
        UniswapV2Factory::new(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            300,
            10000835,
        )
        .into(),
        // UniswapV3
        UniswapV3Factory::new(
            address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
            12369621,
        )
        .into(),
        // Your Factory
        YourFactoryType::new().into(),
    ];

    let _state_space_manager = StateSpaceBuilder::new(provider.clone())
        .with_factories(factories)
        .sync()
        .await?;

```