use std::{error::Error, sync::Arc};

use ethers::providers::{Http, Provider};

use damms::discovery::factory::{discover_factories, DiscoverableFactory};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let factories_filename = "fantom_factories.json";
    let number_of_amms_threshold = 10;

    //Add rpc endpoint here:
    let rpc_endpoint =
        std::env::var("ARBITRUM_MAINNET_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");

    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

    let factories = discover_factories(
        vec![
            DiscoverableFactory::UniswapV2Factory,
            DiscoverableFactory::UniswapV3Factory,
            DiscoverableFactory::IziSwapFactory,
        ],
        number_of_amms_threshold,
        provider,
        100000,
    )
    .await?;

    std::fs::write(
        factories_filename,
        serde_json::to_string_pretty(&factories).unwrap(),
    )
    .unwrap();

    Ok(())
}
