use std::sync::Arc;

use alloy::{
    primitives::address, providers::ProviderBuilder, rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

    let client = ClientBuilder::default()
        .layer(ThrottleLayer::new(500, None)?)
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

    // TODO: add docs detailing when these filters are applied, etc.
    // Whitelist filter that only include pools with the specified "pools" addresses or any pool containing the specified "tokens"
    let filters = vec![
        PoolWhitelistFilter::new(vec![address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")]).into(),
        TokenWhitelistFilter::new(vec![address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")])
            .into(),
    ];

    let _state_space_manager = StateSpaceBuilder::new(provider.clone(), factories)
        .with_filters(filters)
        .sync()
        .await;

    Ok(())
}
