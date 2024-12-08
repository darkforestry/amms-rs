use std::sync::Arc;

use alloy::{
    primitives::address, providers::ProviderBuilder, rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;
use amms::{
    amms::{
        uniswap_v2::{UniswapV2Factory, UniswapV2Pool},
        uniswap_v3::{UniswapV3Factory, UniswapV3Pool},
    },
    state_space::StateSpaceBuilder,
    sync,
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
        // UniswapV3
        UniswapV3Factory::new(
            address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
            12369621,
        )
        .into(),
    ];

    let _state_space_manager = sync!(factories, provider);

    Ok(())
}
