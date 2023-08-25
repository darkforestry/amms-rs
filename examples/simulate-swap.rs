use amms::amm::{uniswap_v2::UniswapV2Pool, AutomatedMarketMaker};
use ethers::{
    providers::{Http, Provider},
    types::{H160, U256},
};
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    // Initialize the pool
    let pool_address = H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?;
    let pool = UniswapV2Pool::new_from_address(pool_address, 300, middleware.clone()).await?;

    // Simulate a swap
    let token_in = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")?;
    let amount_out = pool.simulate_swap(token_in, U256::from_dec_str("1000000000000000000")?)?;

    println!("Amount out: {amount_out}");

    Ok(())
}
