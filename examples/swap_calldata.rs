use alloy::eips::BlockId;
use alloy::primitives::U256;
use alloy::{
    primitives::address, providers::ProviderBuilder, rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;
use amms::amms::{amm::AutomatedMarketMaker, uniswap_v2::UniswapV2Pool};
use std::sync::Arc;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let client = ClientBuilder::default()
        .layer(ThrottleLayer::new(500, None)?)
        .layer(RetryBackoffLayer::new(5, 200, 330))
        .http(rpc_endpoint.parse()?);

    let provider = Arc::new(ProviderBuilder::new().on_client(client));

    let pool = UniswapV2Pool::new(address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"), 3000)
        .init(BlockId::latest(), provider)
        .await?;

    let to_address = address!("DecafC0ffee15BadDecafC0ffee15BadDecafC0f");
    let swap_calldata = pool.swap_calldata(U256::from(10000), U256::ZERO, to_address, vec![]);

    println!("Swap calldata: {:?}", swap_calldata);

    Ok(())
}
