pub mod cache;
#[cfg(feature = "artemis")]
pub mod collector;
pub mod error;

use crate::{
    amm::{AutomatedMarketMaker, AMM},
    errors::EventLogError,
};
use alloy::{
    network::Network,
    primitives::{Address, FixedBytes},
    providers::Provider,
    rpc::types::eth::{Block, Filter, Log},
    transports::Transport,
};
use cache::StateChangeCache;
use error::StateSpaceError;
use futures::StreamExt;
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
};
use tokio::{
    sync::{
        mpsc::{Receiver, Sender},
        RwLock,
    },
    task::JoinHandle,
};

// TODO: bench this with a dashmap
#[derive(Debug)]
pub struct StateSpace(pub HashMap<Address, AMM>);

impl StateSpace {
    pub fn new() -> Self {
        StateSpace(HashMap::new())
    }
}

impl Deref for StateSpace {
    type Target = HashMap<Address, AMM>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StateSpace {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<AMM>> for StateSpace {
    fn from(amms: Vec<AMM>) -> Self {
        let state_space = amms.into_iter().map(|amm| (amm.address(), amm)).collect();
        StateSpace(state_space)
    }
}

#[derive(Debug)]
pub struct StateSpaceManager<T, N, P> {
    state: Arc<RwLock<StateSpace>>,
    state_change_cache: Arc<RwLock<StateChangeCache>>,
    provider: Arc<P>,
    phantom: PhantomData<(T, N)>,
}

// TODO: Much of this can be simplified
impl<T, N, P> StateSpaceManager<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + 'static,
{
    pub fn new(amms: Vec<AMM>, provider: Arc<P>) -> Self {
        Self {
            state: Arc::new(RwLock::new(amms.into())),
            state_change_cache: Arc::new(RwLock::new(StateChangeCache::new())),
            provider,
            phantom: PhantomData,
        }
    }

    pub async fn filter(&self) -> Filter {
        let event_signatures = self
            .state
            .read()
            .await
            .values()
            .flat_map(|amm| amm.sync_on_event_signatures())
            .collect::<Vec<FixedBytes<32>>>();

        Filter::new().event_signature(event_signatures)
    }

    /// Listens to new blocks and handles state changes, sending a Vec<H160> containing each AMM address that incurred a state change in the block.
    pub async fn subscribe_state_changes(
        &self,
        latest_synced_block: u64,
        buffer: usize,
    ) -> Result<
        (
            Receiver<Vec<Address>>,
            Vec<JoinHandle<Result<(), StateSpaceError>>>,
        ),
        StateSpaceError,
    > {
        let (stream_rx, stream_handle) = self.subscribe_blocks_buffered(buffer).await;

        let (sync_amms_rx, sync_amms_handle) = self
            .subscribe_sync_amms(latest_synced_block, stream_rx, buffer)
            .await;

        Ok((sync_amms_rx, vec![stream_handle, sync_amms_handle]))
    }

    async fn subscribe_blocks_buffered(
        &self,
        buffer: usize,
    ) -> (Receiver<Block>, JoinHandle<Result<(), StateSpaceError>>) {
        let (stream_tx, stream_rx): (Sender<Block>, Receiver<Block>) =
            tokio::sync::mpsc::channel(buffer);

        let provider = self.provider.clone();
        let stream_handle = tokio::spawn(async move {
            let subscription = provider.subscribe_blocks().await?;
            let mut block_stream = subscription.into_stream();
            while let Some(block) = block_stream.next().await {
                stream_tx.send(block).await?;
            }

            Ok::<(), StateSpaceError>(())
        });

        (stream_rx, stream_handle)
    }

    pub async fn subscribe_sync_amms(
        &self,
        mut latest_synced_block: u64,
        mut stream_rx: Receiver<Block>,
        buffer: usize,
    ) -> (
        Receiver<Vec<Address>>,
        JoinHandle<Result<(), StateSpaceError>>,
    ) {
        let state = self.state.clone();
        let provider = self.provider.clone();
        let filter = self.filter().await;
        let state_change_cache = self.state_change_cache.clone();

        let (amms_updated_tx, amms_updated_rx) = tokio::sync::mpsc::channel(buffer);

        let updated_amms_handle: JoinHandle<Result<(), StateSpaceError>> =
            tokio::spawn(async move {
                while let Some(block) = stream_rx.recv().await {
                    let chain_head_block_number = block
                        .header
                        .number
                        .ok_or_else(|| StateSpaceError::BlockNumberNotFound)?;

                    // If the chain head block number <= latest synced block, a reorg has occurred
                    if chain_head_block_number <= latest_synced_block {
                        tracing::trace!(
                            chain_head_block_number,
                            latest_synced_block,
                            "reorg detected, unwinding state changes"
                        );

                        latest_synced_block = unwind_state_changes(
                            state.clone(),
                            state_change_cache.clone(),
                            chain_head_block_number,
                        )
                        .await;
                    }

                    // Get logs from the provider that match the event signatures from the state space
                    let logs = provider
                        .get_logs(
                            &filter
                                .clone()
                                .from_block(latest_synced_block + 1)
                                .to_block(chain_head_block_number),
                        )
                        .await?;

                    // Handle any state changes from the logs
                    if !logs.is_empty() {
                        let amms_updated = handle_state_changes_from_logs(
                            state.clone(),
                            state_change_cache.clone(),
                            logs,
                        )
                        .await?;

                        amms_updated_tx.send(amms_updated).await?;
                    }

                    // Once all amms are synced, update the latest synced block
                    latest_synced_block = chain_head_block_number;
                }

                Ok::<(), StateSpaceError>(())
            });

        (amms_updated_rx, updated_amms_handle)
    }
}

#[derive(Debug, Clone)]
pub struct StateChange {
    pub state_change: Vec<AMM>,
    pub block_number: u64,
}

impl StateChange {
    pub fn new(state_change: Vec<AMM>, block_number: u64) -> Self {
        Self {
            block_number,
            state_change,
        }
    }
}

pub async fn handle_state_changes_from_logs(
    state: Arc<RwLock<StateSpace>>,
    state_change_cache: Arc<RwLock<StateChangeCache>>,
    logs: Vec<Log>,
) -> Result<Vec<Address>, StateSpaceError> {
    // If there are no logs to process, return early
    let Some(log) = logs.first() else {
        return Ok(vec![]);
    };

    // Track the block number for the most recently processed
    // log to determine when to commit state changes to cache
    let mut last_log_block_number = get_block_number_from_log(log)?;

    let mut prev_state = vec![];
    let mut updated_amms = HashSet::new();

    // For each log, check if the log is from an amm in the state space and sync the updates
    for log in logs.into_iter() {
        let log_block_number = get_block_number_from_log(&log)?;

        let log_address = log.address();
        if let Some(amm) = state.write().await.get_mut(&log_address) {
            updated_amms.insert(log_address);

            // Push the state of the amm before syncing to cache and then update the state
            prev_state.push(amm.clone());
            amm.sync_from_log(log)?;
        }

        // If the block number has changed, commit the state changes to the cache
        if log_block_number != last_log_block_number {
            commit_state_changes(
                &mut prev_state,
                last_log_block_number,
                state_change_cache.clone(),
            )
            .await;

            last_log_block_number = log_block_number;
        }
    }

    // Commit the state changes for the last block
    commit_state_changes(&mut prev_state, last_log_block_number, state_change_cache).await;

    // Return the addresses of the amms that were affected
    Ok(updated_amms.into_iter().collect())
}

/// Commits state changes contained in `prev_state` to the state change cache
/// and clears the `prev_state` vec
async fn commit_state_changes(
    prev_state: &mut Vec<AMM>,
    block_number: u64,
    state_change_cache: Arc<RwLock<StateChangeCache>>,
) {
    if !prev_state.is_empty() {
        let state_change = StateChange::new(prev_state.clone(), block_number);

        let _ = state_change_cache
            .write()
            .await
            .add_state_change_to_cache(state_change);
    };
    prev_state.clear();
}

/// Unwinds the state changes up to the specified block number
async fn unwind_state_changes(
    state: Arc<RwLock<StateSpace>>,
    state_change_cache: Arc<RwLock<StateChangeCache>>,
    chain_head_block_number: u64,
) -> u64 {
    let updated_amms = state_change_cache
        .write()
        .await
        .unwind_state_changes(chain_head_block_number);

    let mut state_writer = state.write().await;
    for amm in updated_amms {
        state_writer.insert(amm.address(), amm);
    }

    chain_head_block_number - 1
}

/// Extracts the block number from a log
pub fn get_block_number_from_log(log: &Log) -> Result<u64, EventLogError> {
    if let Some(block_number) = log.block_number {
        Ok(block_number)
    } else {
        Err(EventLogError::LogBlockNumberNotFound)
    }
}
