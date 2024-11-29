use std::sync::Arc;

use alloy::{
    primitives::address,
    providers::{ProviderBuilder, WsConnect},
    pubsub::PubSubFrontend,
    rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use futures::StreamExt;
use pamms::{
    amms::{uniswap_v2::UniswapV2Factory, uniswap_v3::UniswapV3Factory},
    state_space::StateSpaceBuilder,
    ThrottleLayer,
};

// TODO: add another example that shows how to maintain sync without pubsub provider
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
        .await;

    let ws_endpoint = std::env::var("ETHEREUM_WSS_ENDPOINT")?;
    let ws = WsConnect::new(ws_endpoint);
    let stream_provider = Arc::new(ProviderBuilder::new().on_ws(ws).await?);

    // Subscribe to state changes
    let mut stream = state_space_manager.subscribe(stream_provider).await.take(5);
    while let Some(state_changes) = stream.next().await {
        dbg!(state_changes);
    }

    Ok(())
}
