use alloy::{
    primitives::address,
    providers::ProviderBuilder,
    rpc::client::ClientBuilder,
    transports::layers::{RetryBackoffLayer, ThrottleLayer},
};
use amms::{amms::uniswap_v2::UniswapV2Factory, state_space::StateSpaceBuilder};
use futures::StreamExt;
use std::sync::Arc;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;
    let client = ClientBuilder::default()
        .layer(ThrottleLayer::new(500))
        .layer(RetryBackoffLayer::new(5, 200, 330))
        .http(rpc_endpoint.parse()?);

    let sync_provider = Arc::new(ProviderBuilder::new().on_client(client));

    let factories = vec![UniswapV2Factory::new(
        address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
        300,
        10000835,
    )
    .into()];

    let state_space_manager = StateSpaceBuilder::new(sync_provider.clone())
        .with_factories(factories)
        .sync()
        .await?;

    /*
    The subscribe method listens for new blocks and fetches
    all logs matching any `sync_events()` specified by the AMM variants in the state space.
    Under the hood, this method applies all state changes to any affected AMMs and returns a Vec of
    addresses, indicating which AMMs have been updated.
    */
    let mut stream = state_space_manager.subscribe().await?.take(5);
    while let Some(updated_amms) = stream.next().await {
        if let Ok(amms) = updated_amms {
            println!("Updated AMMs: {:?}", amms);
        }
    }

    Ok(())
}
