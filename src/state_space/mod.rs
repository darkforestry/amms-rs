pub mod cache;
pub mod discovery;
pub mod filters;

use crate::amms::amm::AutomatedMarketMaker;
use crate::amms::amm::AMM;
use crate::amms::factory::Factory;
use alloy::consensus::ReceiptEnvelope;
use alloy::network::Ethereum;
use alloy::pubsub::PubSubFrontend;
use alloy::pubsub::Subscription;
use alloy::rpc::types::state;
use alloy::rpc::types::Block;
use alloy::rpc::types::BlockTransactions;
use alloy::rpc::types::FilterSet;
use alloy::rpc::types::Header;
use alloy::rpc::types::Log;
use alloy::rpc::types::TransactionReceipt;
use alloy::{
    network::Network,
    primitives::{Address, FixedBytes},
    providers::Provider,
    rpc::types::Filter,
    transports::Transport,
};
use async_stream::stream;
use cache::StateChangeCache;
use derive_more::derive::{Deref, DerefMut};
use discovery::DiscoveryManager;
use eyre::Error;
use futures::stream;
use futures::stream::FuturesUnordered;
use futures::Stream;
use futures::StreamExt;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use std::collections::HashSet;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use std::sync::RwLock;

pub const CACHE_SIZE: usize = 30;

pub struct StateSpaceManager<T, N, P> {
    pub provider: Arc<P>,
    // TODO: think about making the state space a trait, so we can have different implementations and bench whatever is best?
    pub state: Arc<RwLock<StateSpace>>,
    // NOTE: explore more efficient rw locks
    state_change_cache: Arc<RwLock<StateChangeCache<CACHE_SIZE>>>,
    // NOTE: does this need to be atomic u64?
    latest_block: u64,
    discovery_manager: Option<DiscoveryManager>,
    pub block_filter: Filter,
    // TODO: add support for caching
    phantom: PhantomData<(T, N)>,
    // TODO: think about making cache trait then we could experiment with different implementations
}

// NOTE: make it so that you can also just invoke the function to process a block and return the state space
// so that you can invoke it manually with the stream rather than subscribing

impl<T, N, P> StateSpaceManager<T, N, P>
where
    T: Transport + Clone,
    P: Provider<PubSubFrontend> + 'static,
{
    pub async fn subscribe<S>(&mut self) -> impl Stream<Item = Vec<Address>> {
        // Subscribe to the block stream
        let mut block_stream = self.provider.subscribe_blocks().await.expect("TODO:");
        // Clone resources needed for processing
        let state = Arc::clone(&self.state);
        let state_change_cache = Arc::clone(&self.state_change_cache);
        let block_filter = self.block_filter.clone();
        let latest_block = Arc::new(tokio::sync::Mutex::new(self.latest_block)); // Thread-safe `latest_block`
        let provider = Arc::clone(&self.provider);
        // NOTE: think through the best way to do this, whether  getting logs from different provider or the same one
        // Return a stream that processes blocks
        stream! {
            let mut stream = block_stream.into_stream();
            while let Some(t) = stream.next().await {
                yield Self::process_block(t, block_filter.clone(), provider.clone(), state.clone()).await;
            }
        }
    }

    // TODO: function to manually process logs, allowing for
    async fn process_block(
        block: Block,
        block_filter: Filter,
        provider: Arc<P>,
        state: Arc<RwLock<StateSpace>>,
    ) -> Vec<Address> {
        let receipts = provider
            .get_block_receipts(block.header.hash.into())
            .await
            .expect("TODO:");
        if let Some(receipts) = receipts {
            return receipts
                .iter()
                .map(|r| {
                    let inner = &r.inner;
                    let logs = inner
                        .logs()
                        .iter()
                        .filter_map(|l| {
                            if block_filter
                                .topics
                                .iter()
                                .any(|t| t.matches(l.topic0().unwrap_or_default()))
                                && block_filter.address.matches(&l.address())
                            {
                                Some(l.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    state.write().unwrap().sync_logs(logs.clone());
                    logs.iter().map(|l| l.address()).collect::<Vec<_>>()
                })
                .flatten()
                .collect();
        }
        vec![]
    }
}

// NOTE: Drop impl, create a checkpoint

#[derive(Debug, Default)]
pub struct StateSpaceBuilder<T, N, P> {
    // TODO: do we want to add optional amms? for example, if someone wants to sync specific pools but does not care about discovering pools.
    pub provider: Arc<P>,
    pub latest_block: u64,
    pub factories: Vec<Factory>,
    // NOTE: this is the list of filters each discovered pool will go through
    // pub filters: Vec<Filter>,
    pub discovery: bool,
    phantom: PhantomData<(T, N)>,
    // TODO: add support for caching
    // TODO: add support to load from cache
}

impl<T, N, P> StateSpaceBuilder<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<PubSubFrontend, N> + 'static,
{
    pub fn new(provider: Arc<P>, factories: Vec<Factory>) -> StateSpaceBuilder<T, N, P> {
        Self {
            provider,
            latest_block: 0,
            factories,
            discovery: false,
            phantom: PhantomData,
        }
    }

    pub fn block(self, latest_block: u64) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            latest_block,
            ..self
        }
    }

    // NOTE: if you only want to listen to specfic pools, you can add whitelist filter
    // pub fn with_filters(self, filters: Vec<Filter>) -> StateSpaceBuilder<T, N, P> {
    //     StateSpaceBuilder { filters, ..self }
    // }

    pub fn with_discovery(self) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            discovery: true,
            ..self
        }
    }

    pub async fn sync(self) -> StateSpaceManager<T, N, P> {
        let chain_tip = self.provider.get_block_number().await.expect("TODO:");

        let mut futures = FuturesUnordered::new();
        let factories = self.factories.clone();
        for factory in factories {
            let provider = self.provider.clone();

            // TODO: probably also need to specify latest block to sync to
            futures.push(tokio::spawn(async move {
                factory.discovery_sync(chain_tip, provider).await
            }));
        }

        let mut state_space = StateSpace::default();
        while let Some(res) = futures.next().await {
            let amms = res.expect("TODO:").expect("TODO:");

            for amm in amms {
                // println!("Adding AMM: {:?}", amm.address());
                state_space.insert(amm.address(), amm);
            }
        }

        // TODO: filter amms with specified filters

        let mut filter_set = HashSet::new();
        for factory in &self.factories {
            for event in factory.pool_events() {
                filter_set.insert(event);
            }

            if self.discovery {
                filter_set.insert(factory.discovery_event());
            }
        }

        let discovery_manager = if self.discovery {
            Some(DiscoveryManager::new(self.factories))
        } else {
            None
        };

        let block_filter = Filter::new().event_signature(FilterSet::from(
            filter_set.into_iter().collect::<Vec<FixedBytes<32>>>(),
        ));

        StateSpaceManager {
            provider: self.provider,
            latest_block: chain_tip,
            state: Arc::new(RwLock::new(state_space)),
            state_change_cache: Arc::new(RwLock::new(StateChangeCache::default())),
            discovery_manager,
            block_filter,
            phantom: PhantomData,
        }
    }
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct StateSpace(HashMap<Address, AMM>);

impl StateSpace {
    pub fn sync_logs(&mut self, logs: Vec<Log>) {
        for log in logs {
            let address = log.address();
            if let Some(amm) = self.get_mut(&address) {
                amm.sync(log);
            }
        }
    }
}
