use std::sync::Arc;

use alloy::{
    primitives::{address, U256},
    providers::ProviderBuilder,
};

use amms::amm::uniswap_v2::UniswapV2Pool;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse()?));

    // Initialize the pool
    let pool_address = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
    let pool = UniswapV2Pool::new_from_address(pool_address, 300, provider.clone()).await?;

    // Generate the swap calldata
    let to_address = address!("DecafC0ffee15BadDecafC0ffee15BadDecafC0f");
    let swap_calldata = pool.swap_calldata(U256::from(10000), U256::ZERO, to_address, vec![]);

    println!("Swap calldata: {:?}", swap_calldata);

    Ok(())
}
