use std::sync::Arc;

use alloy::{
    primitives::{address, U256},
    providers::ProviderBuilder,
};

use amms::amm::{uniswap_v2::UniswapV2Pool, AutomatedMarketMaker};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse()?));

    // Initialize the pool
    let pool_address = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"); // WETH/USDC
    let pool = UniswapV2Pool::new_from_address(pool_address, 300, provider.clone()).await?;

    // Simulate a swap
    let token_in = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let amount_out = pool.simulate_swap(token_in, U256::from(1000000000000000000_u128))?;

    println!("Amount out: {amount_out}");

    Ok(())
}
