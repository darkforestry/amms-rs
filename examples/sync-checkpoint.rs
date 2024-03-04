use amms::{
    amm::{factory::Factory, uniswap_v2::{factory::UniswapV2Factory, UniswapV2Pool}, uniswap_v3::{factory::UniswapV3Factory, UniswapV3Pool}, AMM},
    state_space::StateSpaceManager,
    sync::checkpoint::{self, sync_amms_from_checkpoint},
};
use ethers::{
    providers::{Http, Middleware, Provider, Ws},
    types::H160,
};
use std::{str::FromStr, sync::Arc};

#[tokio::main]

async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let checkpoint_path = "checkpoint.json";
    let _ = create_checkpoint(checkpoint_path).await;
    let _ = sync_from_checkpoint(checkpoint_path).await;

    Ok(())
}
async fn create_checkpoint(checkpoint_path: &str) -> eyre::Result<()> {
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let ws_endpoint = std::env::var("ETHEREUM_WS_ENDPOINT")?;

    // Initialize middleware
    let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
    let stream_middleware = Arc::new(Provider::<Ws>::connect(ws_endpoint).await?);

    // Initialize factories
    let factories = vec![
        //Add UniswapV2
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")?,
            2638438,
            300,
        )),
        //Add UniswapV3
        Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984")?,
            185,
        )),
    ];
    // Initialize the pool
    let mut amms = vec![];
    let current_block = middleware.get_block_number().await?.as_u64();
    // add uniswap v2 pool
    let pool_address = H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?; // WETH/USDC
    let pool = UniswapV2Pool::new_from_address_to_block(pool_address, 300, current_block, middleware.clone()).await?;
    amms.push(AMM::UniswapV2Pool(pool));
    // add uniswap v3 pool
    let pool_address = H160::from_str("0x7bea39867e4169dbe237d55c8242a8f2fcdcc387")?; // WETH/USDC 1% pool
    let pool = UniswapV3Pool::new_from_address_to_block(pool_address, 10000000, current_block, stream_middleware.clone()).await?;
    amms.push(AMM::UniswapV3Pool(pool));
    // add uniswap v3 pool
    // let pool_address = H160::from_str("0x11b815efb8f581194ae79006d24e0d814b7697f6")?; // WETH/USDT 0.05% pool
    // let pool = UniswapV3Pool::new_from_address_to_block(pool_address, 10000000, current_block, stream_middleware.clone()).await?;
    // amms.push(AMM::UniswapV3Pool(pool));

    // save the checkpoint
    checkpoint::construct_checkpoint(
        factories,
        &amms,
        current_block,
        checkpoint_path,
    )?;

    Ok(())
}

async fn sync_from_checkpoint(checkpoint_path: &str) -> eyre::Result<()> {
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let ws_endpoint = std::env::var("ETHEREUM_WS_ENDPOINT")?;

    // Initialize middleware
    let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
    let stream_middleware = Arc::new(Provider::<Ws>::connect(ws_endpoint).await?);

    // Sync pairs
    let (latest_synced_block, _factories, amms) = sync_amms_from_checkpoint(checkpoint_path, stream_middleware.clone()).await?;
    

    // Initialize state space manager
    let state_space_manager = StateSpaceManager::new(
        amms,
        latest_synced_block,
        100,
        100,
        middleware,
        stream_middleware,
    );

    //Listen for state changes and print them out
    let (mut rx, _join_handles) = state_space_manager.subscribe_state_changes().await?;

    for _ in 0..10 {
        if let Some(state_changes) = rx.recv().await {
            println!("State changes: {:?}", state_changes);
        }
    }

    Ok(())
}

