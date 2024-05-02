// TODO: re-integrate Artemis once its migrated to Alloy
compile_error!("Artemis does not yet support Alloy");

use artemis_core::types::{Collector, CollectorStream};
use async_trait::async_trait;
use ethers::{
    providers::{Middleware, PubsubClient},
    types::H160,
};
use futures::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

use super::StateSpaceManager;

/// # Examples
///
/// use amms::{
///     amm::{
///         factory::Factory, uniswap_v2::factory::UniswapV2Factory,
///         uniswap_v3::factory::UniswapV3Factory, AutomatedMarketMaker, AMM,
///     },
///     state_space::{StateSpace, StateSpaceManager},
///     sync,
/// };
///
/// use artemis_core::{engine, types};
/// use async_trait::async_trait;
/// use ethers::{
///     providers::{Http, Provider, Ws},
///     types::{Transaction, H160},
/// };
/// use std::{collections::HashMap, ops::Deref, str::FromStr, sync::Arc};
/// use tokio::sync::RwLock;
///
/// #[tokio::main]
/// async fn main() -> eyre::Result<()> {
///     tracing_subscriber::fmt::init();
///     let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
///     let ws_endpoint = std::env::var("ETHEREUM_WS_ENDPOINT")?;
///     let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
///     let stream_middleware: Arc<Provider<Ws>> =
///         Arc::new(Provider::<Ws>::connect(ws_endpoint).await?);
///
///     let factories = vec![
///         //Add UniswapV2
///         Factory::UniswapV2Factory(UniswapV2Factory::new(
///             H160::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")?,
///             2638438,
///             300,
///         )),
///         //Add Sushiswap
///         Factory::UniswapV2Factory(UniswapV2Factory::new(
///             H160::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac")?,
///             10794229,
///             300,
///         )),
///         //Add UniswapV3
///         Factory::UniswapV3Factory(UniswapV3Factory::new(
///             H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984")?,
///             185,
///         )),
///     ];
///
///     //Sync amms
///     let (amms, last_synced_block) =
///         sync::sync_amms(factories, middleware.clone(), None, 10000).await?;
///
///     //Initialize state space manager
///     let state_space_manager = StateSpaceManager::new(
///         amms,
///         last_synced_block,
///         100,
///         100,
///         middleware.clone(),
///         stream_middleware,
///     );
///
///     // Group amm addresses by token pairs
///     let pairs = aggregate_pairs(state_space_manager.state.read().await.deref());
///
///     let simple_arbitrage_strategy = SimpleArbitrage {
///         state_space: state_space_manager.state.clone(),
///         pairs,
///     };
///
///     let mut engine: engine::Engine<Vec<H160>, Transaction> = engine::Engine::new();
///     engine.add_collector(Box::new(state_space_manager));
///     engine.add_strategy(Box::new(simple_arbitrage_strategy));
///
///     //Start the engine
///     if let Ok(mut set) = engine.run().await {
///         while let Some(res) = set.join_next().await {
///             tracing::warn!(?res);
///         }
///     }
///     Ok(())
/// }
///
/// pub fn aggregate_pairs(state_space: &StateSpace) -> HashMap<(H160, H160), Vec<H160>> {
///     let mut pairs: HashMap<(H160, H160), Vec<H160>> = HashMap::new();
///
///     for (amm_address, amm) in state_space {
///         let tokens = amm.tokens();
///
///         // This assumes that all pairs only have two tokens for simplicity of the example
///         let (token_a, token_b) = if tokens[0] < tokens[1] {
///             (tokens[0], tokens[1])
///         } else {
///             (tokens[1], tokens[0])
///         };
///
///         let pair = (token_a, token_b);
///
///         if let Some(pair_addresses) = pairs.get_mut(&pair) {
///             pair_addresses.push(*amm_address);
///         } else {
///             pairs.insert(pair, vec![*amm_address]);
///         }
///     }
///
///     pairs
/// }
///
/// struct SimpleArbitrage {
///     state_space: Arc<RwLock<StateSpace>>,
///     pairs: HashMap<(H160, H160), Vec<H160>>,
/// }
///
/// #[async_trait]
/// impl types::Strategy<Vec<H160>, Transaction> for SimpleArbitrage {
///     async fn sync_state(&mut self) -> anyhow::Result<()> {
///         Ok(())
///     }
///
///     async fn process_event(&mut self, event: Vec<H160>) -> Vec<Transaction> {
///         for addr in event {
///             let state_space = self.state_space.read().await;
///
///             let amm: &AMM = state_space
///                 .get(&addr)
///                 // We can expect here because we know the address is from the state space collector
///                 .expect("Could not find amm in Statespace");
///
///             let tokens = amm.tokens();
///
///             let token_a = tokens[0];
///             let token_b = tokens[1];
///
///             let pair_key = if token_a < token_b {
///                 (token_a, token_b)
///             } else {
///                 (token_b, token_a)
///             };
///
///             if let Some(pair_addresses) = self.pairs.get(&pair_key) {
///                 let transactions = vec![];
///
///                 for amm_address in pair_addresses {
///                     let target_amm = state_space
///                         .get(amm_address)
///                         // We can expect here because we know the address is from the state space collector
///                         .expect("Could not find amm in Statespace");
///                     let amm_weight_0 = amm.calculate_price(token_a).unwrap();
///                     let amm_weight_1 = amm.calculate_price(token_b).unwrap();
///
///                     let target_amm_weight_0 = target_amm.calculate_price(token_a).unwrap();
///                     let target_amm_weight_1 = target_amm.calculate_price(token_b).unwrap();
///
///                     if amm_weight_0 * target_amm_weight_1 > 1_f64 {
///                         tracing::info!(to = ?addr, from = ?amm_address, token_in = ?token_a, "Arb detected");
///                     }
///                     if amm_weight_1 * target_amm_weight_0 > 1_f64 {
///                         tracing::info!(to = ?addr, from = ?amm_address, token_in = ?token_b, "Arb detected");
///                     }
///                 }
///
///                 return transactions;
///             }
///         }
///
///         vec![]
///     }
/// }

#[async_trait]
impl<M, P> Collector<Vec<H160>> for StateSpaceManager<M, P>
where
    M: Middleware,
    M::Error: 'static,
    P: Middleware + 'static,
    P::Provider: PubsubClient,
{
    /// Artemis collector implementation for state space manager.
    ///
    /// Returns a `CollectorStream` of `Vec<H160>` representing the AMM addresses that incurred a state change in the block.
    async fn get_event_stream(&self) -> anyhow::Result<CollectorStream<'_, Vec<H160>>> {
        let (state_change_rx, mut join_handles) = self.subscribe_state_changes().await?;

        let stream_handle = join_handles.swap_remove(0);
        let state_change_handle = join_handles.swap_remove(0);

        let early_handle_exit = async move {
            tokio::select! {
                result = stream_handle => {
                  if let Err(e) = result {
                      tracing::error!(?e, "Stream buffer exited early");
                  }
                },
                result = state_change_handle => {
                    if let Err(e) = result {
                        tracing::error!(?e, "State change handler exited early");
                    }
                }
            }
        };

        let stream = ReceiverStream::new(state_change_rx).take_until(early_handle_exit);

        Ok(Box::pin(stream) as CollectorStream<'_, Vec<H160>>)
    }
}
