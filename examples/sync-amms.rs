use std::sync::Arc;

use alloy::{primitives::address, providers::ProviderBuilder};

use amms::{
    amm::{
        balancer::factory::BalancerFactory, factory::Factory,
        uniswap_v2::factory::UniswapV2Factory, uniswap_v3::factory::UniswapV3Factory,
    },
    sync,
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    // Add rpc endpoint here:
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse()?));

    let factories = vec![
        // Add Balancer
        Factory::BalancerFactory(BalancerFactory {
            address: address!("9424B1412450D0f8Fc2255FAf6046b98213B76Bd"),
            creation_block: 9562480,
        }),
        // Add UniswapV2
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            2638438,
            300,
        )),
        // Add Sushiswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            address!("C0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"),
            10794229,
            300,
        )),
        // Add UniswapV3
        Factory::UniswapV3Factory(UniswapV3Factory::new(
            address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
            12369621,
        )),
    ];

    // Sync pairs
    sync::sync_amms(factories, provider, None, 10000000).await?;

    Ok(())
}
