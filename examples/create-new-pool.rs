use std::{error::Error, str::FromStr, sync::Arc};

use ethers::{
    providers::{Http, Provider},
    types::H160,
};

use cfmms::{dex::DexVariant, pool::Pool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Add rpc endpoint here:
    let rpc_endpoint = std::env::var("ETHEREUM_MAINNET_ENDPOINT")
        .expect("Could not get ETHEREUM_MAINNET_ENDPOINT");
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

    //UniswapV2 usdc weth pool on Eth mainnet
    let _uniswap_v2_usdc_weth_pool = Pool::new_from_address(
        H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc").unwrap(),
        DexVariant::UniswapV2,
        provider.clone(),
    )
    .await?;

    //UniswapV3 usdc weth pool on Eth mainnet
    let _uniswap_v3_usdc_weth_pool = Pool::new_from_address(
        H160::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640").unwrap(),
        DexVariant::UniswapV3,
        provider.clone(),
    )
    .await?;

    Ok(())
}
