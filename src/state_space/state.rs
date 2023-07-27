use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use arraydeque::ArrayDeque;
use damms::{
    amm::{AutomatedMarketMaker, AMM},
    errors::EventLogError,
};
use ethers::{
    prelude::gas_oracle::middleware,
    providers::{spoof::State, Middleware, PubsubClient, StreamExt},
    types::{Block, Filter, Log, H160, H256},
};
use tokio::{
    sync::{
        mpsc::{Receiver, Sender},
        RwLock,
    },
    task::JoinHandle,
};

use crate::error::{StateChangeError, StateSpaceError};

pub type StateSpace = HashMap<H160, AMM>;
pub type StateChangeCache = ArrayDeque<StateChange, 150>;

pub trait MiddlewarePubsub: Middleware {
    type PubsubProvider: 'static + PubsubClient;
}

impl<T> MiddlewarePubsub for T
where
    T: Middleware,
    T::Provider: 'static + PubsubClient,
{
    type PubsubProvider = T::Provider;
}

#[derive(Debug)]
pub struct StateSpaceManager<M, P>
where
    M: 'static + Middleware,
    P: 'static + MiddlewarePubsub,
{
    pub state: Arc<RwLock<StateSpace>>,
    pub state_change_cache: Arc<RwLock<StateChangeCache>>,
    pub middleware: Arc<M>,
    pub stream_middleware: Arc<P>,
}

impl<M, P> StateSpaceManager<M, P>
where
    M: Middleware,
    P: MiddlewarePubsub,
{
    pub fn new(amms: Vec<AMM>, middleware: Arc<M>, stream_middleware: Arc<P>) -> Self {
        let state: HashMap<H160, AMM> = amms
            .into_iter()
            .map(|amm| (amm.address(), amm))
            .collect::<HashMap<H160, AMM>>();

        Self {
            state: Arc::new(RwLock::new(state)),
            state_change_cache: Arc::new(RwLock::new(ArrayDeque::new())),
            middleware,
            stream_middleware,
        }
    }

    pub async fn get_block_filter(&self) -> Filter {
        let mut event_signatures: Vec<H256> = vec![];
        let mut amm_variants = HashSet::new();

        for amm in self.state.read().await.values() {
            let variant = match amm {
                AMM::UniswapV2Pool(_) => 0,
                AMM::UniswapV3Pool(_) => 1,
                AMM::ERC4626Vault(_) => 2,
                AMM::IziSwapPool(_) => 3,
            };

            if !amm_variants.contains(&variant) {
                amm_variants.insert(variant);
                event_signatures.extend(amm.sync_on_event_signatures());
            }
        }

        //Create a new filter
        Filter::new().topic0(event_signatures)
    }

    //listens to new blocks and handles state changes, sending an h256 block hash when a new block is produced
    //pub fn listen_for_new_blocks()-> Result<Receiver<H256>, StateSpaceError<M>> {}
    pub async fn listen_for_updates(
        &self,
        mut last_synced_block: u64,
        channel_buffer: usize,
    ) -> Result<Vec<JoinHandle<Result<(), StateSpaceError<M, P>>>>, StateSpaceError<M, P>>
    where
        <P as Middleware>::Provider: PubsubClient,
    {
        let state = self.state.clone();
        let _state_change_cache: StateChangeCache = ArrayDeque::new();
        let middleware = self.middleware.clone();
        let stream_middleware: Arc<P> = self.stream_middleware.clone();
        let filter = self.get_block_filter().await;

        let (stream_tx, mut stream_rx): (Sender<Block<H256>>, Receiver<Block<H256>>) =
            tokio::sync::mpsc::channel(channel_buffer);

        let stream_handle = tokio::spawn(async move {
            let mut block_stream = stream_middleware
                .subscribe_blocks()
                .await
                .map_err(StateSpaceError::PubsubClientError)?;

            while let Some(block) = block_stream.next().await {
                stream_tx.send(block).await?;
            }

            Ok::<(), StateSpaceError<M, P>>(())
        });

        let state_change_cache = self.state_change_cache.clone();
        let new_block_handle: JoinHandle<Result<(), StateSpaceError<M, P>>> =
            tokio::spawn(async move {
                while let Some(block) = stream_rx.recv().await {
                    if let Some(chain_head_block_number) = block.number {
                        let chain_head_block_number = chain_head_block_number.as_u64();

                        //If there is a reorg, unwind state changes from last_synced block to the chain head block number
                        if chain_head_block_number <= last_synced_block {
                            unwind_state_changes(
                                state.clone(),
                                state_change_cache.clone(),
                                chain_head_block_number,
                            )
                            .await?;

                            //TODO: update this comment to explain why we set it to -1
                            last_synced_block = chain_head_block_number - 1;
                        }

                        let from_block: u64 = last_synced_block + 1;
                        let logs = middleware
                            .get_logs(
                                &filter
                                    .clone()
                                    .from_block(from_block)
                                    .to_block(chain_head_block_number),
                            )
                            .await
                            .map_err(StateSpaceError::MiddlewareError)?;

                        if logs.is_empty() {
                            for block_number in from_block..=chain_head_block_number {
                                add_state_change_to_cache(
                                    state_change_cache.clone(),
                                    StateChange::new(None, block_number),
                                )
                                .await?;
                            }
                        } else {
                            handle_state_changes_from_logs(
                                state.clone(),
                                state_change_cache.clone(),
                                logs,
                                middleware.clone(),
                            )
                            .await?;
                        }

                        last_synced_block = chain_head_block_number;
                    } else {
                        return Err(StateSpaceError::BlockNumberNotFound);
                    }
                }

                Ok::<(), StateSpaceError<M, P>>(())
            });

        Ok(vec![stream_handle, new_block_handle])
    }

    //listens to new blocks and handles state changes, sending an h256 block hash when a new block is produced
    //pub fn listen_for_new_blocks()-> Result<Receiver<H256>, StateSpaceError<M>> {}
    pub async fn listen_for_new_blocks(
        &self,
        mut last_synced_block: u64,
        channel_buffer: usize,
    ) -> Result<
        (
            Receiver<Block<H256>>,
            Vec<JoinHandle<Result<(), StateSpaceError<M, P>>>>,
        ),
        StateSpaceError<M, P>,
    >
    where
        <P as Middleware>::Provider: PubsubClient,
    {
        let state = self.state.clone();
        let _state_change_cache: StateChangeCache = ArrayDeque::new();
        let middleware = self.middleware.clone();
        let stream_middleware: Arc<P> = self.stream_middleware.clone();
        let filter = self.get_block_filter().await;

        let (stream_tx, mut stream_rx): (Sender<Block<H256>>, Receiver<Block<H256>>) =
            tokio::sync::mpsc::channel(channel_buffer);

        let stream_handle = tokio::spawn(async move {
            let mut block_stream = stream_middleware
                .subscribe_blocks()
                .await
                .map_err(StateSpaceError::PubsubClientError)?;

            while let Some(block) = block_stream.next().await {
                stream_tx.send(block).await?;
            }

            Ok::<(), StateSpaceError<M, P>>(())
        });

        let (new_block_tx, new_block_rx) = tokio::sync::mpsc::channel(channel_buffer);

        let state_change_cache = self.state_change_cache.clone();
        let new_block_handle: JoinHandle<Result<(), StateSpaceError<M, P>>> =
            tokio::spawn(async move {
                while let Some(block) = stream_rx.recv().await {
                    if let Some(chain_head_block_number) = block.number {
                        let chain_head_block_number = chain_head_block_number.as_u64();

                        //If there is a reorg, unwind state changes from last_synced block to the chain head block number
                        if chain_head_block_number <= last_synced_block {
                            unwind_state_changes(
                                state.clone(),
                                state_change_cache.clone(),
                                chain_head_block_number,
                            )
                            .await?;

                            //TODO: update this comment to explain why we set it to -1
                            last_synced_block = chain_head_block_number - 1;
                        }

                        let from_block: u64 = last_synced_block + 1;
                        let logs = middleware
                            .get_logs(
                                &filter
                                    .clone()
                                    .from_block(from_block)
                                    .to_block(chain_head_block_number),
                            )
                            .await
                            .map_err(StateSpaceError::MiddlewareError)?;

                        if logs.is_empty() {
                            for block_number in from_block..=chain_head_block_number {
                                add_state_change_to_cache(
                                    state_change_cache.clone(),
                                    StateChange::new(None, block_number),
                                )
                                .await?;
                            }
                        } else {
                            handle_state_changes_from_logs(
                                state.clone(),
                                state_change_cache.clone(),
                                logs,
                                middleware.clone(),
                            )
                            .await?;
                        }

                        last_synced_block = chain_head_block_number;

                        new_block_tx.send(block).await?;
                    } else {
                        return Err(StateSpaceError::BlockNumberNotFound);
                    }
                }

                Ok::<(), StateSpaceError<M, P>>(())
            });

        Ok((new_block_rx, vec![stream_handle, new_block_handle]))
    }

    pub async fn listen_for_state_changes(
        &self,
        mut last_synced_block: u64,
        channel_buffer: usize,
    ) -> Result<
        (
            Receiver<Vec<H160>>,
            Vec<JoinHandle<Result<(), StateSpaceError<M, P>>>>,
        ),
        StateSpaceError<M, P>,
    >
    where
        <P as Middleware>::Provider: PubsubClient,
    {
        let state = self.state.clone();
        let _state_change_cache: StateChangeCache = ArrayDeque::new();
        let middleware = self.middleware.clone();
        let stream_middleware: Arc<P> = self.stream_middleware.clone();
        let filter = self.get_block_filter().await;

        let (stream_tx, mut stream_rx): (Sender<Block<H256>>, Receiver<Block<H256>>) =
            tokio::sync::mpsc::channel(channel_buffer);

        let stream_handle = tokio::spawn(async move {
            let mut block_stream = stream_middleware
                .subscribe_blocks()
                .await
                .map_err(StateSpaceError::PubsubClientError)?;

            while let Some(block) = block_stream.next().await {
                stream_tx.send(block).await?;
            }

            Ok::<(), StateSpaceError<M, P>>(())
        });

        let (amms_updated_tx, amms_updated_rx) = tokio::sync::mpsc::channel(channel_buffer);

        let state_change_cache = self.state_change_cache.clone();

        let updated_amms_handle: JoinHandle<Result<(), StateSpaceError<M, P>>> =
            tokio::spawn(async move {
                while let Some(block) = stream_rx.recv().await {
                    if let Some(chain_head_block_number) = block.number {
                        let chain_head_block_number = chain_head_block_number.as_u64();

                        //If there is a reorg, unwind state changes from last_synced block to the chain head block number
                        if chain_head_block_number <= last_synced_block {
                            unwind_state_changes(
                                state.clone(),
                                state_change_cache.clone(),
                                chain_head_block_number,
                            )
                            .await?;

                            //set the last synced block to the head block number
                            last_synced_block = chain_head_block_number - 1;
                        }

                        let from_block: u64 = last_synced_block + 1;
                        let logs = middleware
                            .get_logs(
                                &filter
                                    .clone()
                                    .from_block(from_block)
                                    .to_block(chain_head_block_number),
                            )
                            .await
                            .map_err(StateSpaceError::MiddlewareError)?;

                        if logs.is_empty() {
                            for block_number in from_block..=chain_head_block_number {
                                add_state_change_to_cache(
                                    state_change_cache.clone(),
                                    StateChange::new(None, block_number),
                                )
                                .await?;
                            }
                        } else {
                            let amms_updated = handle_state_changes_from_logs(
                                state.clone(),
                                state_change_cache.clone(),
                                logs,
                                middleware.clone(),
                            )
                            .await?;

                            amms_updated_tx.send(amms_updated).await?;
                        }

                        last_synced_block = chain_head_block_number;
                    } else {
                        return Err(StateSpaceError::BlockNumberNotFound);
                    }
                }

                Ok::<(), StateSpaceError<M, P>>(())
            });

        Ok((amms_updated_rx, vec![stream_handle, updated_amms_handle]))
    }
}

pub fn initialize_state_space(amms: Vec<AMM>) -> StateSpace {
    amms.into_iter()
        .map(|amm| (amm.address(), amm))
        .collect::<HashMap<H160, AMM>>()
}

#[derive(Debug)]
pub struct StateChange {
    pub state_change: Option<Vec<AMM>>,
    pub block_number: u64,
}

impl StateChange {
    pub fn new(state_change: Option<Vec<AMM>>, block_number: u64) -> Self {
        Self {
            block_number,
            state_change,
        }
    }
}

//Unwinds the state changes cache for every block from the most recent state change cache back to the block to unwind -1
async fn unwind_state_changes(
    state: Arc<RwLock<StateSpace>>,
    state_change_cache: Arc<RwLock<StateChangeCache>>,
    block_to_unwind: u64,
) -> Result<(), StateChangeError> {
    let mut state_change_cache = state_change_cache.write().await;

    //TODO: update this to use a range and not a loop
    loop {
        //check if the most recent state change block is >= the block to unwind,
        if let Some(state_change) = state_change_cache.get(0) {
            if state_change.block_number >= block_to_unwind {
                if let Some(option_state_changes) = state_change_cache.pop_front() {
                    if let Some(state_changes) = option_state_changes.state_change {
                        for amm_state in state_changes {
                            state.write().await.insert(amm_state.address(), amm_state);
                        }
                    }
                } else {
                    //We know that there is a state change from state_change_cache.get(0) so when we pop front without returning a value, there is an issue
                    return Err(StateChangeError::PopFrontError);
                }
            } else {
                return Ok(());
            }
        } else {
            //We return an error here because we never want to be unwinding past where we have state changes.
            //For example, if you initialize a state space that syncs to block 100, then immediately after there is a chain reorg to 95, we can not roll back the state
            //changes for an accurate state space. In this case, we return an error
            return Err(StateChangeError::NoStateChangesInCache);
        }
    }
}

async fn add_state_change_to_cache(
    state_change_cache: Arc<RwLock<StateChangeCache>>,
    state_change: StateChange,
) -> Result<(), StateChangeError> {
    let mut state_change_cache = state_change_cache.write().await;

    if state_change_cache.is_full() {
        state_change_cache.pop_back();
        state_change_cache
            .push_front(state_change)
            .map_err(|_| StateChangeError::CapacityError)?
    } else {
        state_change_cache
            .push_front(state_change)
            .map_err(|_| StateChangeError::CapacityError)?
    }
    Ok(())
}

pub async fn handle_state_changes_from_logs<M: Middleware>(
    state: Arc<RwLock<StateSpace>>,
    state_change_cache: Arc<RwLock<StateChangeCache>>,
    logs: Vec<Log>,
    middleware: Arc<M>,
) -> Result<Vec<H160>, StateChangeError> {
    let mut updated_amms_set = HashSet::new();
    let mut updated_amms = vec![];
    let mut state_changes = vec![];

    let mut last_log_block_number = if let Some(log) = logs.get(0) {
        get_block_number_from_log(log)?
    } else {
        return Ok(updated_amms);
    };

    for log in logs.into_iter() {
        let log_block_number = get_block_number_from_log(&log)?;

        // check if the log is from an amm in the state space
        if let Some(amm) = state.write().await.get_mut(&log.address) {
            if !updated_amms_set.contains(&log.address) {
                updated_amms_set.insert(log.address);
                updated_amms.push(log.address);
            }

            state_changes.push(amm.clone());
            //TODO: uncomment this when all sync from log functions are not async
            // amm.sync_from_log(log)?;

            match amm {
                AMM::UniswapV2Pool(pool) => pool.sync_from_log(log)?,
                AMM::UniswapV3Pool(pool) => pool.sync_from_log(log)?,
                AMM::ERC4626Vault(vault) => vault.sync_from_log(log)?,
                AMM::IziSwapPool(pool) => pool
                    .sync_from_swap_log(log, middleware.clone())
                    .await
                    .map_err(|_| StateChangeError::MiddlewareError)?,
            }
        }

        //Commit state changes if the block has changed since last log
        if log_block_number != last_log_block_number {
            if state_changes.is_empty() {
                add_state_change_to_cache(
                    state_change_cache.clone(),
                    StateChange::new(None, last_log_block_number),
                )
                .await?;
            } else {
                add_state_change_to_cache(
                    state_change_cache.clone(),
                    StateChange::new(Some(state_changes), last_log_block_number),
                )
                .await?;
                state_changes = vec![];
            };

            last_log_block_number = log_block_number;
        }
    }

    if state_changes.is_empty() {
        add_state_change_to_cache(
            state_change_cache,
            StateChange::new(None, last_log_block_number),
        )
        .await?;
    } else {
        add_state_change_to_cache(
            state_change_cache,
            StateChange::new(Some(state_changes), last_log_block_number),
        )
        .await?;
    };

    Ok(updated_amms)
}

pub fn get_block_number_from_log(log: &Log) -> Result<u64, EventLogError> {
    if let Some(block_number) = log.block_number {
        Ok(block_number.as_u64())
    } else {
        Err(damms::errors::EventLogError::LogBlockNumberNotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::{default, sync::Arc};

    use damms::amm::{uniswap_v2::UniswapV2Pool, AMM};
    use ethers::{
        providers::{Http, Provider, Ws},
        types::H160,
    };
    use tokio::sync::RwLock;

    use super::StateSpaceManager;
    use crate::state::{
        add_state_change_to_cache, unwind_state_changes, StateChange, StateChangeCache,
    };

    #[tokio::test]
    async fn test_add_state_changes() {
        let state_change_cache = Arc::new(RwLock::new(StateChangeCache::new()));

        //TODO: update to emulate state changes from block range
        for i in 0..=100 {
            let new_amm = AMM::UniswapV2Pool(UniswapV2Pool {
                address: H160::zero(),
                reserve_0: i,
                ..default::Default::default()
            });

            add_state_change_to_cache(
                state_change_cache.clone(),
                StateChange::new(Some(vec![new_amm]), i as u64),
            )
            .await
            .expect("could not add state change");
        }

        let mut state_change_cache = state_change_cache.write().await;
        //TODO: deconstruct this cleaner
        if let Some(last_state_change) = state_change_cache.pop_front() {
            if let Some(state_changes) = last_state_change.state_change {
                assert_eq!(state_changes.len(), 1);

                if let AMM::UniswapV2Pool(pool) = &state_changes[0] {
                    assert_eq!(pool.reserve_0, 100);
                } else {
                    panic!("Unexpected AMM variant")
                }
            } else {
                panic!("state changes not found")
            }
        }
    }

    #[tokio::test]
    async fn test_unwind_state_changes() {
        // let rpc_endpoint =
        //     std::env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");
        // let ws_endpoint =
        //     std::env::var("ETHEREUM_WS_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");

        let middleware = Arc::new(Provider::<Http>::try_from("").unwrap());

        let stream_middleware = Arc::new(
            Provider::<Ws>::connect("")
                .await
                .expect("could not initialize ws provider"),
        );

        let amms = vec![AMM::UniswapV2Pool(UniswapV2Pool {
            address: H160::zero(),
            ..default::Default::default()
        })];

        let state_space_manager = StateSpaceManager::new(amms, middleware, stream_middleware);
        let state_change_cache = Arc::new(RwLock::new(StateChangeCache::new()));

        for i in 0..100 {
            let new_amm = AMM::UniswapV2Pool(UniswapV2Pool {
                address: H160::zero(),
                reserve_0: i,
                ..default::Default::default()
            });

            add_state_change_to_cache(
                state_change_cache.clone(),
                StateChange::new(Some(vec![new_amm]), i as u64),
            )
            .await
            .expect("could not add state change");
        }

        unwind_state_changes(state_space_manager.state, state_change_cache, 50)
            .await
            .expect("could not unwind state changes");

        //TODO: assert state changes
    }

    #[tokio::test]
    async fn test_add_empty_state_changes() {
        let last_synced_block = 0;
        let chain_head_block_number = 100;

        let state_change_cache = Arc::new(RwLock::new(StateChangeCache::new()));

        for block_number in last_synced_block..=chain_head_block_number {
            add_state_change_to_cache(
                state_change_cache.clone(),
                StateChange::new(None, block_number),
            )
            .await
            .expect("could not add state change");
        }

        let state_change_cache_length = state_change_cache.read().await.len();
        assert_eq!(state_change_cache_length, 101)
    }
}
