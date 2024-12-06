use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
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

    let block_number = provider.get_block_number().await?;
    let pool = UniswapV2Pool::new(address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"))
        .init(block_number, provider)
        .await?;

    let amount_out = pool.simulate_swap(pool.token_a, Address::default(), U256::from(1000000))?;
    println!("Amount out: {:?}", amount_out);

    Ok(())
}
