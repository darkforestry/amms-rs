use std::sync::Arc;

use alloy::{
    primitives::address, providers::ProviderBuilder, rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;
use amms::{
    amms::{amm::AutomatedMarketMaker, uniswap_v2::UniswapV2Factory},
    state_space::StateSpaceBuilder,
};
use futures::StreamExt;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let client = ClientBuilder::default()
        .layer(ThrottleLayer::new(500, None)?)
        .layer(RetryBackoffLayer::new(5, 200, 330))
        .http(rpc_endpoint.parse()?);

    let sync_provider = Arc::new(ProviderBuilder::new().on_client(client));

    let factories = vec![
        // UniswapV2
        UniswapV2Factory::new(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            300,
            10000835,
        )
        .into(),
    ];

    let state_space_manager = StateSpaceBuilder::new(sync_provider.clone(), factories)
        .sync()
        .await?;

    // Subscribe to state changes
    let mut stream = state_space_manager.subscribe().await?.take(5);
    while let Some(updated_amms) = stream.next().await {
        if let Ok(amms) = updated_amms {
            println!("Updated AMMs: {:?}", amms);
        }
    }

    Ok(())
}
