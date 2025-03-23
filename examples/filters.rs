use alloy::{
    primitives::address,
    providers::ProviderBuilder,
    rpc::client::ClientBuilder,
    transports::layers::{RetryBackoffLayer, ThrottleLayer},
};
use amms::{
    amms::uniswap_v2::UniswapV2Factory,
    state_space::{
        filters::whitelist::{PoolWhitelistFilter, TokenWhitelistFilter},
        StateSpaceBuilder,
    },
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

    let client = ClientBuilder::default()
        .layer(ThrottleLayer::new(500))
        .layer(RetryBackoffLayer::new(5, 200, 330))
        .http(rpc_endpoint.parse()?);

    let provider = Arc::new(ProviderBuilder::new().on_client(client));

    let factories = vec![
        // UniswapV2
        UniswapV2Factory::new(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            300,
            10000835,
        )
        .into(),
    ];

    /*  PoolFilters are applied all AMMs when syncing the state space.
       Filters have two "stages", `FilterStage::Discovery` or `FilterStage::Sync`.
       Discovery filters are applied to AMMs after the `StateSpaceManager` has processed all pool created events.
       Sync filters are applied to AMMs after the `StateSpaceManager` has processed all pool sync events.
       This allows for efficient syncing of the state space by minimizing the amount of pools that need to sync state.
       In the following example, the `PoolWhitelistFilter` is applied to the `Discovery` stage
       and the `TokenWhitelistFilter` is applied to the `Sync` stage. Rather than syncing all pools from the factory,
       only the whitelisted pools are synced. The `TokenWhitelistFilter` is applied after syncing since pool creation logs
       do not always emit the tokens included in the pool, but this data will always be populated after syncing.
    */
    let filters = vec![
        PoolWhitelistFilter::new(vec![address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")]).into(),
        TokenWhitelistFilter::new(vec![address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")])
            .into(),
    ];

    let _state_space_manager = StateSpaceBuilder::new(provider.clone())
        .with_factories(factories)
        .with_filters(filters)
        .sync()
        .await;

    Ok(())
}
