use std::sync::Arc;

use alloy::{
    primitives::address, providers::ProviderBuilder, pubsub::PubSubFrontend,
    rpc::client::ClientBuilder, transports::layers::RetryBackoffLayer,
};
use pamms::{
    amms::{uniswap_v2::UniswapV2Factory, uniswap_v3::UniswapV3Factory},
    state_space::StateSpaceBuilder,
    ThrottleLayer,
};

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

    let state_space_manager = StateSpaceBuilder::new(provider.clone(), factories)
        .with_discovery()
        .sync()
        .await;

    // Subscribe to state changes
    let mut stream = state_space_manager.subscribe().take(5);
    while let Some(amms) = stream.next().await {
        dbg!(amms);
    }

    Ok(())
}
