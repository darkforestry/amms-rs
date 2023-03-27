use std::{error::Error, str::FromStr, sync::Arc};

use ethers::{
    providers::{Http, Provider},
    types::H160,
};

use damms::{
    checkpoint::generate_checkpoint,
    dex::{Dex, DexVariant},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Add rpc endpoint here:
    let rpc_endpoint =
        std::env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

    let dexes = vec![
        //Add Sushiswap
        Dex::new(
            H160::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac").unwrap(),
            DexVariant::UniswapV2,
            10794229,
            Some(300),
        ),
    ];

    // Sync pools and generate checkpoint
    generate_checkpoint(dexes, provider.clone(), "checkpoint.json").await?;

    Ok(())
}
