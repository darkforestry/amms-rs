use amms::amm::uniswap_v2::UniswapV2Pool;
use ethers::{
    providers::{Http, Provider},
    types::{H160, U256},
};
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    // Initialize the pool
    let pool_address = H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?;
    let pool = UniswapV2Pool::new_from_address(pool_address, 300, middleware.clone()).await?;

    // Generate the swap calldata
    let to_address = H160::from_str("0xcoffee")?;
    let swap_calldata = pool.swap_calldata(U256::from(10000), U256::zero(), to_address, vec![]);

    println!("Swap calldata: {:?}", swap_calldata);

    Ok(())
}
