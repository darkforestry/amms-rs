use std::{error::Error, sync::Arc};

use ethers::providers::{Http, Provider};

use damms::discovery::factory::{discover_factories, DiscoverableFactory};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let factories_filename = "iotex_factories.json";
    let number_of_amms_threshold = 50;

    //Add rpc endpoint here:
    let rpc_endpoint = "https://rpc.ankr.com/iotex/c922039faf67b4cd4df65d69244b9562caf4477652c7d9d6654b8e3ecc38b445";

    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

    let factories = discover_factories(
        vec![
            DiscoverableFactory::UniswapV2Factory,
            DiscoverableFactory::UniswapV3Factory,
        ],
        number_of_amms_threshold,
        provider,
        1000,
    )
    .await?;

    std::fs::write(
        factories_filename,
        serde_json::to_string_pretty(&factories).unwrap(),
    )
    .unwrap();

    Ok(())
}
