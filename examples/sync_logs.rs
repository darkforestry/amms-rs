use std::sync::{atomic::Ordering, Arc};

use alloy::{
    primitives::address,
    providers::{Provider, ProviderBuilder},
    rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;
use amms::{amms::uniswap_v2::UniswapV2Factory, state_space::StateSpaceBuilder};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
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
        .sync()
        .await;
    let state = state_space_manager.state;

    let mut latest_block = state_space_manager.latest_block.load(Ordering::Relaxed);
    for _ in 0..5 {
        let current_block = provider.get_block_number().await?;

        let block_filter = state_space_manager
            .block_filter
            .clone()
            .from_block(latest_block)
            .to_block(current_block);

        let logs = provider.get_logs(&block_filter).await?;
        let affected_amms = state.write().await.sync(&logs);

        dbg!(affected_amms);

        latest_block = current_block;
        tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
    }

    Ok(())
}
