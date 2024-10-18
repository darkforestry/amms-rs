pub mod cache;
pub mod discovery;
pub mod filters;

use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use crate::amms::amm::AutomatedMarketMaker;
use crate::amms::amm::AMM;
use crate::amms::factory::AutomatedMarketMakerFactory;
use crate::amms::factory::Factory;
use crate::amms::uniswap_v2::UniswapV2Pool;
use alloy::rpc::types::FilterSet;
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
use tokio::sync::RwLock;

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
        let discovery_manager = DiscoveryManager::new(self.factories.unwrap_or_default());

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

        let mut block_filter = Filter::new().event_signature(FilterSet::from(
            filter_set.into_iter().collect::<Vec<FixedBytes<32>>>(),
        ));

        // TODO: implement a batch contract for getting all token decimals?

        let mut state_space = StateSpace::default();
        let state_change_cache = StateChangeCache::<30>::new();
        let token_decimals = HashMap::<Address, usize>::new();

        let chain_tip = self
            .provider
            .get_block_number()
            .await
            .expect("TODO: handle error");

        while self.latest_block <= chain_tip {
            let next_block = self.latest_block + 1;
            block_filter = block_filter.from_block(next_block);
            block_filter = block_filter.to_block(next_block + self.sync_step);

            let logs = self
                .provider
                .get_logs(&block_filter)
                .await
                .expect("TODO: handle error");

            // NOTE: get all events by step

            for log in logs {
                if let Some(factory) = discovery_manager.factories.get(&log.address()) {
                    let pool = factory.create_pool(log).expect("handle errors");
                    state_space.state.insert(pool.address(), pool);
                } else if let Some(amm) = state_space.state.get_mut(&log.address()) {
                    // NOTE: update pool
                    amm.sync(log);
                }
            }

            // NOTE: Check if event is from factories and create new pool.
            // NOTE: if not, check if event is from existing pool in state space and update accordlingly

            self.latest_block += self.sync_step;
        }

        // TODO: filter amms based on specified filters

        todo!();
        // NOTE: before passing in the block filter if discovery is not enabled, we need to remove discovery events from the block filter

        // StateSpaceManager {
        //     provider: self.provider,
        //     latest_block: self.latest_block,
        //     state: Arc::new(RwLock::new(StateSpace::default())),
        //     state_change_cache: Arc::new(RwLock::new(StateChangeCache::new())),
        //     discovery_manager,
        //     block_filter,
        //     phantom: PhantomData,
        // }
    }
}

#[derive(Debug, Default)]
pub struct StateSpace {
    // NOTE: bench dashmap instead
    state: HashMap<Address, AMM>,
}
