use std::sync::Arc;

use alloy::{primitives::address, providers::ProviderBuilder};
use pamms::{
    amms::{uniswap_v2::UniswapV2Factory, uniswap_v3::UniswapV3Factory},
    state_space::StateSpaceBuilder,
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    // Add rpc endpoint here:
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;
    let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse()?));

    let factories = vec![
        // UniswapV2
        // UniswapV2Factory::new(
        //     address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
        //     300,
        //     10000835,
        // )
        // .into(),
        // Sushiswap
        UniswapV2Factory::new(
            address!("C0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"),
            300,
            10794229,
        )
        .into(),
        // UniswapV3Factory::new(
        //     address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
        //     12369621,
        // )
        // .into(),
    ];

    let state_space_manager: pamms::state_space::StateSpaceManager<
        alloy::transports::http::Http<alloy::transports::http::Client>,
        alloy::network::Ethereum,
        alloy::providers::RootProvider<
            alloy::transports::http::Http<alloy::transports::http::Client>,
        >,
    > = StateSpaceBuilder::new(provider.clone())
        .with_factories(factories)
        .with_discovery()
        // .with_filters()
        // .block(123456)
        .sync_step(2000)
        .with_throttle(5)
        .sync()
        .await;

    Ok(())
}
