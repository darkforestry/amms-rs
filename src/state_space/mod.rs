pub mod cache;
pub mod discovery;
pub mod error;
pub mod filters;

use crate::amms::amm::AutomatedMarketMaker;
use crate::amms::amm::AMM;
use crate::amms::error::AMMError;
use crate::amms::factory::Factory;

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::rpc::types::Block;
use alloy::rpc::types::FilterSet;
use alloy::rpc::types::Log;
use alloy::{
    network::Network,
    primitives::{Address, FixedBytes},
    providers::Provider,
    rpc::types::Filter,
    transports::Transport,
};
use async_stream::stream;
use cache::StateChange;
use cache::StateChangeCache;

use error::StateSpaceError;
use filters::AMMFilter;
use filters::PoolFilter;
use futures::stream::FuturesUnordered;
use futures::Stream;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::min;
use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use tokio::runtime::Handle;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;

pub const CACHE_SIZE: usize = 30;

#[derive(Clone)]
pub struct StateSpaceManager<T, N, P> {
    pub state: Arc<RwLock<StateSpace>>,
    pub latest_block: Arc<AtomicU64>,
    // discovery_manager: Option<DiscoveryManager>,
    pub block_filter: Filter,
    pub provider: Arc<P>,
    pub checkpoint_path: Option<PathBuf>,
    phantom: PhantomData<(T, N)>,
}

impl<T, N, P> StateSpaceManager<T, N, P> {
    pub async fn subscribe(
        &self,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<Vec<Address>, StateSpaceError>> + Send>>,
        StateSpaceError,
    >
    where
        P: Provider<T, N> + 'static,
        T: Transport + Clone,
        N: Network<BlockResponse = Block>,
    {
        let provider = self.provider.clone();
        let latest_block = self.latest_block.clone();
        let state = self.state.clone();
        let mut block_filter = self.block_filter.clone();

        let block_stream = provider.subscribe_blocks().await?.into_stream();

        Ok(Box::pin(stream! {
            tokio::pin!(block_stream);

            while let Some(block) = block_stream.next().await {
                let block_number = block.number();
                block_filter = block_filter.select(block_number);


                let logs = provider.get_logs(&block_filter).await?;

                let affected_amms = state.write().await.sync(&logs)?;
                latest_block.store(block_number, Ordering::Relaxed);

                yield Ok(affected_amms);
            }
        }))
    }

    pub async fn write_checkpoint(&self) {
        self.checkpoint_path
            .as_ref()
            .map(|path| async move { self.state.read().await.write_checkpoint(path.clone()) });
    }
}

impl<T, N, P> Drop for StateSpaceManager<T, N, P> {
    fn drop(&mut self) {
        let rt = Handle::current();
        rt.block_on(self.write_checkpoint());
    }
}

#[derive(Debug, Default)]
pub struct StateSpaceBuilder<T, N, P> {
    pub provider: Arc<P>,
    pub latest_block: u64,
    pub factories: Vec<Factory>,
    pub amms: Vec<AMM>,
    pub filters: Vec<PoolFilter>,
    phantom: PhantomData<(T, N)>,
    pub checkpoint_path: Option<PathBuf>,
    pub discovery: bool, // TODO:
}

impl<T, N, P> StateSpaceBuilder<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + 'static,
{
    pub fn new(provider: Arc<P>) -> StateSpaceBuilder<T, N, P> {
        Self {
            provider,
            latest_block: 0,
            factories: vec![],
            amms: vec![],
            filters: vec![],
            discovery: false,
            phantom: PhantomData,
            checkpoint_path: None,
        }
    }

    pub fn block(self, latest_block: u64) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            latest_block,
            ..self
        }
    }

    pub fn with_factories(self, factories: Vec<Factory>) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder { factories, ..self }
    }

    pub fn with_amms(self, amms: Vec<AMM>) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder { amms, ..self }
    }

    pub fn with_filters(self, filters: Vec<PoolFilter>) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder { filters, ..self }
    }

    pub fn with_checkpoint(self, path: impl AsRef<Path>) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            checkpoint_path: Some(path.as_ref().canonicalize().unwrap()),
            ..self
        }
    }

    pub fn load_checkpoint(mut self) -> StateSpaceBuilder<T, N, P> {
        if let Some(path) = self.checkpoint_path.as_ref() {
            let state_space: StateSpace = serde_json::from_reader(File::open(path).expect(
                "Failed to open checkpoint file. Ensure the path is correct and you have read permissions.",
            )).expect("Failed to deserialize checkpoint file");
            self.latest_block = state_space.latest_block.load(Ordering::Relaxed);
            self.amms.extend(state_space.state.into_values());
        };
        self
    }

    pub async fn sync(self) -> Result<StateSpaceManager<T, N, P>, StateSpaceError> {
        let this = self.load_checkpoint();
        let chain_tip = BlockId::from(this.provider.get_block_number().await?);
        let factories = this.factories.clone();
        let mut futures = FuturesUnordered::new();

        let mut amm_variants = HashMap::new();
        for amm in this.amms.into_iter() {
            amm_variants
                .entry(amm.variant())
                .or_insert_with(Vec::new)
                .push(amm);
        }

        for factory in factories {
            let provider = this.provider.clone();
            let filters = this.filters.clone();

            let extension = amm_variants.remove(&factory.variant());
            futures.push(tokio::spawn(async move {
                let from_block = if this.latest_block == 0 {
                    None
                } else {
                    Some(this.latest_block.into())
                };
                let mut discovered_amms = factory
                    .discover(from_block, chain_tip, provider.clone())
                    .await?;

                if let Some(amms) = extension {
                    discovered_amms.extend(amms);
                }

                // Apply discovery filters
                for filter in filters.iter() {
                    if filter.stage() == filters::FilterStage::Discovery {
                        let pre_filter_len = discovered_amms.len();
                        discovered_amms = filter.filter(discovered_amms).await?;

                        info!(
                            target: "state_space::sync",
                            factory = %factory.address(),
                            pre_filter_len,
                            post_filter_len = discovered_amms.len(),
                            filter = ?filter,
                            "Discovery filter"
                        );
                    }
                }

                // `discovered_amms` are always empty regardless of checkpoint - sync through batched calls.
                discovered_amms = factory.sync(discovered_amms, chain_tip, provider).await?;

                // Apply sync filters
                for filter in filters.iter() {
                    if filter.stage() == filters::FilterStage::Sync {
                        let pre_filter_len = discovered_amms.len();
                        discovered_amms = filter.filter(discovered_amms).await?;

                        info!(
                            target: "state_space::sync",
                            factory = %factory.address(),
                            pre_filter_len,
                            post_filter_len = discovered_amms.len(),
                            filter = ?filter,
                            "Sync filter"
                        );
                    }
                }

                Ok::<Vec<AMM>, AMMError>(discovered_amms)
            }));
        }

        // Initialize an empty state space.
        let mut state_space = StateSpace::default();
        // Collect all AMMs that are non-empty i.e. have been loaded from the checkpoint.
        let populated_amms = amm_variants
            .iter()
            .flat_map(|(_, amms)| amms.iter().cloned())
            .filter(|amm| amm.initialized())
            .collect::<Vec<AMM>>();
        let unpopulated_amms = amm_variants
            .iter()
            .flat_map(|(_, amms)| amms.iter().cloned())
            .filter(|amm| !amm.initialized())
            .collect::<Vec<AMM>>();

        // Insert the populated AMMs into the state space.
        state_space.state.extend(
            populated_amms
                .iter()
                .map(|amm| (amm.address(), amm.clone())),
        );

        // Sync populated AMMs in the state space from logs self.latest_block + 1 to chain_tip.
        let step = 1000;
        let tip = chain_tip.as_u64().unwrap();
        for i in (this.latest_block + 1..=tip).step_by(step as usize) {
            let filter = Filter::new().from_block(i).to_block(min(i + step, tip));
            let logs = this.provider.get_logs(&filter).await?;
            state_space.sync(&logs)?;
        }

        // Sync unpopulates AMM variants
        for mut amm in unpopulated_amms {
            let address = amm.address();
            amm = amm.init(chain_tip, this.provider.clone()).await?;
            state_space.state.insert(address, amm);
        }

        while let Some(res) = futures.next().await {
            let synced_amms = res??;

            for amm in synced_amms {
                // println!("Adding AMM: {:?}", amm.address());
                state_space.state.insert(amm.address(), amm);
            }
        }

        let mut filter_set = HashSet::new();
        for factory in &this.factories {
            for event in factory.pool_events() {
                filter_set.insert(event);
            }
        }

        let block_filter = Filter::new().event_signature(FilterSet::from(
            filter_set.into_iter().collect::<Vec<FixedBytes<32>>>(),
        ));

        Ok(StateSpaceManager {
            latest_block: Arc::new(AtomicU64::new(this.latest_block)),
            state: Arc::new(RwLock::new(state_space)),
            block_filter,
            provider: this.provider,
            checkpoint_path: this.checkpoint_path,
            phantom: PhantomData,
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StateSpace {
    pub state: HashMap<Address, AMM>,
    pub latest_block: AtomicU64,
    #[serde(skip)]
    cache: StateChangeCache<CACHE_SIZE>,
}

impl StateSpace {
    pub fn get(&self, address: &Address) -> Option<&AMM> {
        self.state.get(address)
    }

    pub fn get_mut(&mut self, address: &Address) -> Option<&mut AMM> {
        self.state.get_mut(address)
    }

    pub fn sync(&mut self, logs: &[Log]) -> Result<Vec<Address>, StateSpaceError> {
        let latest = self.latest_block.load(Ordering::Relaxed);
        let Some(mut block_number) = logs
            .first()
            .map(|log| log.block_number.ok_or(StateSpaceError::MissingBlockNumber))
            .transpose()?
        else {
            return Ok(vec![]);
        };

        // Check if there is a reorg and unwind to state before block_number
        if latest >= block_number {
            info!(
                target: "state_space::sync",
                from = %latest,
                to = %block_number - 1,
                "Unwinding state changes"
            );

            let cached_state = self.cache.unwind_state_changes(block_number);
            for amm in cached_state {
                debug!(target: "state_space::sync", ?amm, "Reverting AMM state");
                self.state.insert(amm.address(), amm);
            }
        }

        let mut cached_amms = HashSet::new();
        let mut affected_amms = HashSet::new();
        for log in logs {
            // If the block number is updated, cache the current block state changes
            let log_block_number = log
                .block_number
                .ok_or(StateSpaceError::MissingBlockNumber)?;
            if log_block_number != block_number {
                let amms = cached_amms.drain().collect::<Vec<AMM>>();
                affected_amms.extend(amms.iter().map(|amm| amm.address()));
                let state_change = StateChange::new(amms, block_number);

                debug!(
                    target: "state_space::sync",
                    state_change = ?state_change,
                    "Caching state change"
                );

                self.cache.push(state_change);
                block_number = log_block_number;
            }

            // If the AMM is in the state space add the current state to cache and sync from log
            let address = log.address();
            if let Some(amm) = self.state.get_mut(&address) {
                cached_amms.insert(amm.clone());
                amm.sync(log)?;

                info!(
                    target: "state_space::sync",
                    ?amm,
                    "Synced AMM"
                );
            }
        }

        if !cached_amms.is_empty() {
            let amms = cached_amms.drain().collect::<Vec<AMM>>();
            affected_amms.extend(amms.iter().map(|amm| amm.address()));
            let state_change = StateChange::new(amms, block_number);

            debug!(
                target: "state_space::sync",
                state_change = ?state_change,
                "Caching state change"
            );

            self.cache.push(state_change);
        }

        Ok(affected_amms.into_iter().collect())
    }

    pub fn write_checkpoint(&self, path: PathBuf) {
        serde_json::to_writer(File::create(path).expect(
            "Failed to create checkpoint file. Ensure the path is correct and you have write permissions.",
        ), self).expect("Failed to serialize state space");
    }
}

#[macro_export]
macro_rules! sync {
    // Sync factories with provider
    ($factories:expr, $provider:expr) => {{
        StateSpaceBuilder::new($provider.clone())
            .with_factories($factories)
            .sync()
            .await?
    }};

    // Sync factories with filters
    ($factories:expr, $filters:expr, $provider:expr) => {{
        StateSpaceBuilder::new($provider.clone())
            .with_factories($factories)
            .with_filters($filters)
            .sync()
            .await?
    }};

    ($factories:expr, $amms:expr, $filters:expr, $provider:expr) => {{
        StateSpaceBuilder::new($provider.clone())
            .with_factories($factories)
            .with_amms($amms)
            .with_filters($filters)
            .sync()
            .await?
    }};
}
