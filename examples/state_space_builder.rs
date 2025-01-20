use std::sync::Arc;

use alloy::{
    primitives::address, providers::ProviderBuilder, rpc::client::ClientBuilder,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;
use amms::{
    amms::{
        erc_4626::ERC4626Vault,
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

    /*
       The `StateSpaceBuilder` is used to sync a state space of AMMs.

       When specifying a set of factories to sync from, the `sync()` method fetches all pool creation logs
       from the factory contracts specified and syncs all pools to the latest block. This method returns a
       `StateSpaceManager` which can be used to subscribe to state changes and interact with AMMs
       the state space.
    */
    let factories = vec![
        // UniswapV2
        UniswapV2Factory::new(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            3000,
            10000835,
        )
        .into(),
        // UniswapV3
        UniswapV3Factory::new(
            address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
            12369621,
        )
        .into(),
    ];

    let _state_space_manager = StateSpaceBuilder::new(provider.clone())
        .with_factories(factories.clone())
        .sync()
        .await?;

    // ======================================================================================== //

    /*
    You can also sync pools directly without specifying factories. This is great for when you only
    need to track a handful of specific pools.
    */
    let amms = vec![
        UniswapV2Pool::new(address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"), 3000).into(),
        UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")).into(),
    ];

    let _state_space_manager = StateSpaceBuilder::new(provider.clone())
        .with_amms(amms)
        .sync()
        .await?;

    // ======================================================================================== //

    /*
    Additionally, you can specify specific factories to discover and sync pools from, as well as
    specify specific AMMs to sync. This can be helpful when there isnt a factory for a given AMM
    as is the case with ERC4626 vaults.
    */
    let amms = vec![ERC4626Vault::new(address!("163538E22F4d38c1eb21B79939f3d2ee274198Ff")).into()];

    let _state_space_manager = StateSpaceBuilder::new(provider.clone())
        .with_amms(amms)
        .sync()
        .await?;

    Ok(())
}
