use amms::{
    amm::{factory::Factory, uniswap_v2::factory::UniswapV2Factory, AMM},
    discovery,
    state_space::state::StateSpaceManager,
    sync,
};
use ethers::{
    providers::{Http, Provider, Ws},
    types::H160,
};
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

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
        //Add Sushiswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac")?,
            10794229,
            300,
        )),
    ];

    let step = 1000;

    //Sync amms
    let (mut amms, last_synced_block) =
        sync::sync_amms(factories, middleware.clone(), None, step).await?;

    // Discover vaults and add them to amms
    let vaults = discovery::erc_4626::discover_erc_4626_vaults(middleware.clone(), step)
        .await?
        .into_iter()
        .map(AMM::ERC4626Vault)
        .collect::<Vec<AMM>>();

    amms.extend(vaults);

    // Initialize state space manager
    let state_space_manager = StateSpaceManager::new(amms, middleware, stream_middleware);

    //Listen for state changes and print them out
    let (mut rx, _join_handles) = state_space_manager
        .listen_for_state_changes(last_synced_block, 100)
        .await?;

    for _ in 0..10 {
        if let Some(state_changes) = rx.recv().await {
            println!("State changes: {:?}", state_changes);
        }
    }

    Ok(())
}
