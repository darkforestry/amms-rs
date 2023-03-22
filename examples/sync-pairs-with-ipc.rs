use std::{str::FromStr, sync::Arc, time::Duration};

use cfmms::{
    dex::{Dex, DexVariant},
    sync,
};
use ethers::{
    providers::{Ipc, Provider},
    types::H160,
};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Add ipc endpoint here:
    let ipc_endpoint = "~/.ethereum/geth.ipc";
    let provider: Arc<Provider<Ipc>> = Arc::new(
        Provider::connect_ipc(ipc_endpoint)
            .await?
            .interval(Duration::from_millis(2000)),
    );

    let dexes = vec![
        //UniswapV2
        Dex::new(
            H160::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap(),
            DexVariant::UniswapV2,
            2638438,
            Some(300),
        ),
        //Add Sushiswap
        Dex::new(
            H160::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac").unwrap(),
            DexVariant::UniswapV2,
            10794229,
            Some(300),
        ),
        //Add UniswapV3
        Dex::new(
            H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),
            DexVariant::UniswapV3,
            12369621,
            None,
        ),
    ];

    //Sync pairs
    sync::sync_pairs(dexes, provider, None).await?;

    Ok(())
}
