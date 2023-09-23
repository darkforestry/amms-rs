use amms::discovery::{
    discovery_options::DiscoveryOptionsBuilder,
    factory::{discover_factories, DiscoverableFactory},
};
use ethers::providers::{Http, Provider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;

    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    // Filter Uniswap V2 compatible factories from block 10000835 to block 10091468
    // expect to get only the original Uniswap V2 Factory
    let options = DiscoveryOptionsBuilder::default()
        .from_block(10000835)
        .to_block(Some(10091468))
        .step(100000)
        .number_of_amms_threshold(5)
        .build()?;

    let factories = discover_factories(
        vec![
            DiscoverableFactory::UniswapV2Factory,
            // DiscoverableFactory::UniswapV3Factory,
        ],
        provider,
        options,
    )
    .await?;

    println!("Factories: {:?}", factories);

    Ok(())
}
