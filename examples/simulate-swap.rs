use amms::amm::{uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool, AutomatedMarketMaker};
use ethers::{
    providers::{Http, Provider, Ws},
    types::{H160, U256},
};
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    uniswap_v2_simulate_swap().await?;
    uniswap_v3_simulate_swap().await?;
    Ok(())
}


async fn uniswap_v2_simulate_swap() -> eyre::Result<()> {
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    // Initialize the pool
    let pool_address = H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?; // WETH/USDC
    let pool = UniswapV2Pool::new_from_address(pool_address, 300, middleware.clone()).await?;

    // Simulate a swap
    let token_in = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")?;
    let amount_out = pool.simulate_swap(token_in, U256::from_dec_str("1000000000000000000")?)?;

    println!("Uniswap V2 simulate swap WETH/USDC, 1 WETH Amount In, swap {amount_out} USDC Amount out");

    Ok(())
}

async fn uniswap_v3_simulate_swap() -> eyre::Result<()> {
    let ws_endpoint = std::env::var("ETHEREUM_WS_ENDPOINT")?;
    let stream_middleware = Arc::new(Provider::<Ws>::connect(ws_endpoint).await?);

    // Initialize the pool
    // 0x7bea39867e4169dbe237d55c8242a8f2fcdcc387 WETH/USDC 1%
    // 0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640 WETH/USDC 0.05%
    let pool_address = H160::from_str("0x7bea39867e4169dbe237d55c8242a8f2fcdcc387")?; // WETH/USDC
    let pool = UniswapV3Pool::new_from_address(pool_address, 10000000, stream_middleware.clone()).await?;

    // Simulate a swap
    let token_in = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")?;
    let amount_out = pool.simulate_swap(token_in, U256::from_dec_str("1000000000000000000")?)?;

    println!("Uniswap V3 simulate swap WETH/USDC 1%, 1 WETH Amount In, swap {amount_out} USDC Amount out");

    Ok(())
}
