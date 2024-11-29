pub mod cache;
pub mod discovery;
pub mod filters;

use crate::amms::amm::AutomatedMarketMaker;
use crate::amms::amm::AMM;
use crate::amms::factory::Factory;

use alloy::network::BlockResponse;
use alloy::pubsub::PubSubFrontend;
use alloy::pubsub::Subscription;
use alloy::pubsub::SubscriptionStream;
use alloy::rpc::types::Block;
use alloy::rpc::types::FilterSet;
use alloy::rpc::types::Header;
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
use derive_more::derive::{Deref, DerefMut};
use discovery::DiscoveryManager;

use filters::PoolFilter;
use futures::stream::FuturesUnordered;
use futures::Stream;
use futures::StreamExt;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::RwLock;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

pub const CACHE_SIZE: usize = 30;

#[derive(Clone)]
pub struct StateSpaceManager {
    pub state: Arc<RwLock<StateSpace>>,
    latest_block: Arc<AtomicU64>,
    // discovery_manager: Option<DiscoveryManager>,
    pub block_filter: Filter,
    // TODO: add support for caching
}

impl StateSpaceManager {
    pub async fn subscribe<S>(
        &self,
        stream_provider: Arc<S>,
    ) -> Pin<Box<dyn Stream<Item = Vec<Address>> + Send>>
    where
        S: Provider<PubSubFrontend> + 'static,
    {
        let latest_block = self.latest_block.clone();
        let state = self.state.clone();
        let mut block_filter = self.block_filter.clone();

        let block_stream = stream_provider
            .subscribe_blocks()
            .await
            .expect("TODO:")
            .into_stream();

        Box::pin(stream! {
            tokio::pin!(block_stream);

            while let Some(block) = block_stream.next().await {
                let block_number = block.header.number;
                block_filter = block_filter.select(block_number);


                let logs = stream_provider
                .get_logs(&block_filter)
                .await
                .expect("TODO:");

                state.write().expect("TODO: handle error").sync(&logs);
                latest_block.store(block_number, Ordering::Relaxed);

                let affected_amms = logs.iter().map(|l| l.address()).collect::<Vec<_>>();
                yield affected_amms;
            }
        })
    }
}

// NOTE: Drop impl, create a checkpoint

#[derive(Debug, Default)]
pub struct StateSpaceBuilder<T, N, P> {
    // TODO: do we want to add optional amms? for example, if someone wants to sync specific pools but does not care about discovering pools.
    pub provider: Arc<P>,
    pub latest_block: u64,
    pub factories: Vec<Factory>,
    pub filters: Vec<PoolFilter>,
    phantom: PhantomData<(T, N)>,
    // TODO: add support for caching
    // TODO: add support to load from cache
}

impl<T, N, P> StateSpaceBuilder<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + 'static,
{
    pub fn new(provider: Arc<P>, factories: Vec<Factory>) -> StateSpaceBuilder<T, N, P> {
        Self {
            provider,
            latest_block: 0,
            factories,
            filters: vec![],
            // discovery: false,
            phantom: PhantomData,
        }
    }

    pub fn block(self, latest_block: u64) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            latest_block,
            ..self
        }
    }

    pub fn with_filters(self, filters: Vec<PoolFilter>) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder { filters, ..self }
    }

    pub async fn sync(self) -> StateSpaceManager {
        let chain_tip = self.provider.get_block_number().await.expect("TODO:");

        let mut futures = FuturesUnordered::new();
        let factories = self.factories.clone();
        for factory in factories {
            let provider = self.provider.clone();

            // TODO: probably also need to specify latest block to sync to
            futures.push(tokio::spawn(async move {
                let amms = factory
                    .discover(chain_tip, provider)
                    .await
                    .expect("TODO: handle error");

                // TODO: NOTE: filter amms with discovery filter stage, then sync and then filter

                amms
            }));
        }

        let mut state_space = StateSpace::default();
        while let Some(res) = futures.next().await {
            let amms = res.expect("TODO:");

            for amm in amms {
                // println!("Adding AMM: {:?}", amm.address());
                state_space.state.insert(amm.address(), amm);
            }
        }

        // TODO: filter amms with specified filters

        let mut filter_set = HashSet::new();
        for factory in &self.factories {
            for event in factory.pool_events() {
                filter_set.insert(event);
            }
        }

        let block_filter = Filter::new().event_signature(FilterSet::from(
            filter_set.into_iter().collect::<Vec<FixedBytes<32>>>(),
        ));

        StateSpaceManager {
            latest_block: Arc::new(AtomicU64::new(self.latest_block)),
            state: Arc::new(RwLock::new(state_space)),
            block_filter,
        }
    }
}

#[derive(Debug, Default)]
// TODO: add cache to state space as a private field do eliminate unnecessary mutex on state space cache
pub struct StateSpace {
    pub state: HashMap<Address, AMM>,
    pub latest_block: Arc<AtomicU64>,
    cache: StateChangeCache<CACHE_SIZE>,
}

impl StateSpace {
    pub fn sync(&mut self, logs: &[Log]) {
        let latest = self.latest_block.load(Ordering::Relaxed);
        let mut block_number = logs
            .first()
            .expect("TODO: handle error")
            .block_number
            .expect("TODO: Handle this");

        // Check if there is a reorg and unwind to state before block_number
        if latest >= block_number {
            let cached_state = self.cache.unwind_state_changes(block_number);
            for amm in cached_state {
                self.state.insert(amm.address(), amm);
            }
        }

        let mut cached_amms = HashSet::new();
        for log in logs {
            // If the block number is updated, cache the current block state changes
            let log_block_number = log.block_number.expect("TODO: Handle this");
            if log_block_number != block_number {
                self.cache.push(StateChange::new(
                    cached_amms.drain().collect(),
                    block_number,
                ));
                block_number = log_block_number;
            }

            // If the AMM is in the state space add the current state to cache and sync from log
            let address = log.address();
            if let Some(amm) = self.state.get_mut(&address) {
                cached_amms.insert(amm.clone());
                amm.sync(log);
            }
        }

        if !cached_amms.is_empty() {
            self.cache.push(StateChange::new(
                cached_amms.drain().collect(),
                block_number,
            ));
        }
    }
}
