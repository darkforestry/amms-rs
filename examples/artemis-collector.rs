use amms::{
    amm::{
        factory::Factory, uniswap_v2::factory::UniswapV2Factory,
        uniswap_v3::factory::UniswapV3Factory,
    },
    state_space::StateSpaceManager,
    sync,
};
use artemis_core::engine::Engine;
use artemis_core::types::Strategy;
use async_trait::async_trait;
use ethers::{
    providers::{Http, Provider, Ws},
    types::{Transaction, H160},
};
use std::{str::FromStr, sync::Arc};
#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let ws_endpoint = std::env::var("ETHEREUM_WS_ENDPOINT")?;
    let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
    let stream_middleware: Arc<Provider<Ws>> =
        Arc::new(Provider::<Ws>::connect(ws_endpoint).await?);

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
        //Add UniswapV3
        Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984")?,
            185,
        )),
    ];

    //Sync amms
    let (amms, last_synced_block) =
        sync::sync_amms(factories, middleware.clone(), None, 500).await?;

    //Initialize state space manager
    let state_space_manager = StateSpaceManager::new(
        amms,
        last_synced_block,
        100,
        100,
        middleware.clone(),
        stream_middleware,
    );

    let mut engine: Engine<Vec<H160>, Transaction> = Engine::new();
    // Add the collector
    engine.add_collector(Box::new(state_space_manager));
    // Add the strategy
    engine.add_strategy(Box::new(DummyStrategy));
    //Start the engine
    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            tracing::info!("res: {:?}", res);
        }
    }
    Ok(())
}

struct DummyStrategy;
#[async_trait]
impl Strategy<Vec<H160>, Transaction> for DummyStrategy {
    async fn sync_state(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Vec<H160>) -> Vec<Transaction> {
        tracing::info!("Processing event: {:?}", event);
        vec![]
    }
}
