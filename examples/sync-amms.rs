use amms::{
    amm::{
        factory::Factory, uniswap_v2::factory::UniswapV2Factory,
        uniswap_v3::factory::UniswapV3Factory,
    },
    sync,
};
use ethers::{
    providers::{Http, Provider},
    types::H160,
};
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    //Add rpc endpoint here:
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    let factories = vec![
        //Add UniswapV2
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")?,
            2638438,
            300,
        )),
        //Add Sushiswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac")?,
            10794229,
            300,
        )),
        //Add UniswapV3
        Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984")?,
            185,
        )),
    ];

    //Sync pairs
    sync::sync_amms(factories, provider, None, 500).await?;

    Ok(())
}
