use std::sync::Arc;

use alloy::{
    primitives::address, providers::ProviderBuilder, rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;
use amms::{
    amms::{
        uniswap_v2::{UniswapV2Factory, UniswapV2Pool},
        uniswap_v3::{UniswapV3Factory, UniswapV3Pool},
    },
    state_space::StateSpaceBuilder,
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

    let client = ClientBuilder::default()
        .layer(ThrottleLayer::new(500, None)?)
        .layer(RetryBackoffLayer::new(5, 200, 330))
        .http(rpc_endpoint.parse()?);

    let provider = Arc::new(ProviderBuilder::new().on_client(client));

    let amms = vec![
        UniswapV2Pool::new(address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"), 300).into(),
        UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")).into(),
    ];

    let _state_space_manager = StateSpaceBuilder::new(provider.clone())
        .with_amms(amms)
        .sync()
        .await?;

    Ok(())
}
