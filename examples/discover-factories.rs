use std::{error::Error, str::FromStr, sync::Arc};

use ethers::{
    providers::{Http, Provider},
    types::H160,
};

use damms::discovery::factory::{discover_factories, DiscoverableFactory};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let factories_filename = "factories.json";
    //Add rpc endpoint here:
    let rpc_endpoint =
        std::env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

    let factories = discover_factories(
        vec![
            DiscoverableFactory::UniswapV2Factory,
            DiscoverableFactory::UniswapV3Factory,
        ],
        50,
        provider,
    )
    .await?;

    std::fs::write(
        factories_filename,
        serde_json::to_string_pretty(&factories).unwrap(),
    )
    .unwrap();

    Ok(())
}
