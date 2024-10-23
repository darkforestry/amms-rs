pub mod cache;
pub mod discovery;
pub mod filters;
pub mod tokens;

use std::ops::Range;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use crate::amms::amm::AutomatedMarketMaker;
use crate::amms::amm::AMM;
use crate::amms::factory::AutomatedMarketMakerFactory;
use crate::amms::factory::Factory;
use alloy::rpc::types::{FilterSet, Log};
use alloy::{
    network::Network,
    primitives::{Address, FixedBytes},
    providers::Provider,
    rpc::types::Filter,
    transports::Transport,
};
use cache::StateChangeCache;
use discovery::DiscoveryManager;
use std::collections::HashSet;
use tokens::populate_token_decimals;
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinSet;

pub const CACHE_SIZE: usize = 30;
pub const TASK_PERMITS: Semaphore = Semaphore::const_new(50);

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

// NOTE: Drop impl, create a checkpoint

#[derive(Debug, Default)]
pub struct StateSpaceBuilder<T, N, P> {
    // TODO: do we want to add optional amms? for example, if someone wants to sync specific pools but does not care about discovering pools.
    pub provider: Arc<P>,
    pub latest_block: u64,
    pub factories: Option<Vec<Factory>>,
    pub amms: Option<Vec<AMM>>,
    // NOTE: this is the list of filters each discovered pool will go through
    // pub filters: Vec<Filter>,
    pub discovery: bool,
    pub sync_step: u64,
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
    pub fn new(provider: Arc<P>) -> StateSpaceBuilder<T, N, P> {
        Self {
            provider,
            latest_block: 0,
            factories: None,
            amms: None,
            discovery: false,
            sync_step: 10000,
            phantom: PhantomData,
        }
    }

    pub fn block(self, latest_block: u64) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            latest_block,
            ..self
        }
    }

    pub fn with_factories(self, factories: Vec<Factory>) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            factories: Some(factories),
            ..self
        }
    }

    pub fn with_amms(self, amms: Vec<AMM>) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            amms: Some(amms),
            ..self
        }
    }

    pub fn sync_step(self, sync_step: u64) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder { sync_step, ..self }
    }

    // pub fn with_filters(self, filters: Vec<Filter>) -> StateSpaceBuilder<T, N, P> {
    //     StateSpaceBuilder { filters, ..self }
    // }

    pub fn with_discovery(self) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            discovery: true,
            ..self
        }
    }

    // TODO: pub fn with_filters(self) -> StateSpaceBuilder<T, N, P> {}

    pub async fn sync(mut self) -> StateSpaceManager<T, N, P> {
        let discovery_manager = DiscoveryManager::new(self.factories.clone().unwrap_or_default());

        // Create an initial filter set with all discovery events for each factory
        let mut filter_set = discovery_manager.disc_events();
        let mut sync_events = HashSet::new();

        // Add pool events to block filter for all factory related amms
        for (_, factory) in discovery_manager.factories.iter() {
            sync_events.extend(factory.pool_events());
        }

        // Add sync events to block filter for all specified amms
        if let Some(amms) = self.amms {
            for amm in amms.iter() {
                sync_events.extend(amm.sync_events());
            }
        }

        // We keep disc events and sync events separate in the case discovery
        // is disabled within the state space manager
        filter_set.extend(sync_events);

        let block_filter = Filter::new().event_signature(FilterSet::from(
            filter_set.into_iter().collect::<Vec<FixedBytes<32>>>(),
        ));

        // TODO: implement a batch contract for getting all token decimals?
        let mut state_space = StateSpace::default();
        let state_change_cache = StateChangeCache::<30>::new();
        let mut tokens = HashSet::new();

        let chain_tip = self
            .provider
            .get_block_number()
            .await
            .expect("TODO: handle error");

        self.latest_block = self.factories.as_ref().map_or(0, |factories| {
            factories
                .iter()
                .map(|factory| factory.creation_block())
                .min()
                .unwrap_or(0)
        });

        // HashMap k=block_range, v=ordered logs
        let mut rng_logs = HashMap::<Range<u64>, Vec<Log>>::new();
        let mut join_set = JoinSet::new();
        tracing::debug!("Syncing from block {} to block {}", self.latest_block, chain_tip);

        while self.latest_block < chain_tip {
            let mut block_filter = block_filter.clone();
            let from_block = self.latest_block;
            let to_block = (from_block + self.sync_step).min(chain_tip);
            block_filter = block_filter.from_block(from_block);
            block_filter = block_filter.to_block(to_block);
            let provider = self.provider.clone();
            self.latest_block = to_block;
            join_set.spawn(async move {
                let _ = TASK_PERMITS.acquire().await.unwrap();
                let logs = provider.get_logs(&block_filter).await?;
                Ok::<(Range<u64>, Vec<Log>), eyre::Report>(((from_block..to_block), logs))
            });
        }

        while let Some(res) = join_set.join_next().await {
            if let Ok(Ok((block_range, logs))) = res {
                tracing::debug!("Got logs for block range {:?}", block_range);
                rng_logs.insert(block_range, logs);
            } else {
                panic!("TODO: handle error");
            }
        }

        let mut keys = rng_logs.keys().into_iter().collect::<Vec<_>>();

        keys.sort_by(|a, b| a.start.cmp(&b.start));

        for key in keys {
            let logs = rng_logs.get(key).unwrap().clone();
            for log in logs {
                if let Some(factory) = discovery_manager.factories.get(&log.address()) {
                    let amm = factory.create_pool(log).expect("handle errors");

                    for token in amm.tokens() {
                        tokens.insert(token);
                    }

                    state_space.state.insert(amm.address(), amm);
                } else if let Some(amm) = state_space.state.get_mut(&log.address()) {
                    amm.sync(log);
                }
            }
        }
        // TODO: This might exceed max gas per static on some clients depending on the chain.
        let token_decimals = populate_token_decimals(tokens, self.provider.clone())
            .await
            .expect("TODO: handle error");

        for (_, amm) in state_space.state.iter_mut() {
            amm.set_decimals(&token_decimals);
        }

        // TODO: filter amms with specified filters

        StateSpaceManager {
            provider: self.provider,
            latest_block: self.latest_block,
            state: Arc::new(RwLock::new(state_space)),
            state_change_cache: Arc::new(RwLock::new(state_change_cache)),
            discovery_manager: Some(discovery_manager),
            block_filter,
            phantom: PhantomData,
        }
    }
}

#[derive(Debug, Default)]

//TODO: maybe just do StateSpace(HashMap<Address,AMM>) and use all inner functions
pub struct StateSpace {
    // NOTE: bench dashmap instead
    state: HashMap<Address, AMM>,
}

impl StateSpace {
    pub fn insert(&mut self, address: Address, pool: AMM) {
        self.state.insert(address, pool);
    }

    pub fn remove(&mut self, address: Address) {
        self.state.remove(&address);
    }

    pub fn get(&self, address: &Address) -> Option<&AMM> {
        self.state.get(address)
    }

    pub fn get_mut(&mut self, address: &Address) -> Option<&mut AMM> {
        self.state.get_mut(address)
    }
}
