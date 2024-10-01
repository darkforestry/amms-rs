pub mod cache;
pub mod discovery;
pub mod filters;

use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use crate::amms::amm::AMM;
use crate::amms::factory::AutomatedMarketMakerFactory;
use crate::amms::factory::Factory;
use alloy::rpc::types::FilterSet;
use alloy::{
    network::Network, primitives::Address, providers::Provider, rpc::types::Filter,
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
        let (disc_events, factory_addresses) = self.factories.as_ref().map_or_else(
            || (HashSet::new(), HashSet::new()),
            |factories| {
                factories.iter().fold(
                    (HashSet::new(), HashSet::new()),
                    |(mut events_set, mut addresses_set), factory| {
                        events_set.extend(factory.discovery_events());
                        addresses_set.insert(factory.address());
                        (events_set, addresses_set)
                    },
                )
            },
        );

        // let block_filter = alloy::rpc::types::Filter::new()
        //     .address(address)
        //     .event_signature(FilterSet::from(disc_events));

        // TODO: implement a batch contract for getting all token decimals?

        //TODO: add all AMM filters to sync filter
        let mut state_space = StateSpace::default();
        let state_change_cache = StateChangeCache::<30>::new();
        let token_decimals = HashMap::<Address, usize>::new();

        // NOTE: TODO: sync through the block range and get all the events
        // NOTE: we can check if the log is a disc event or a sync event and then handle accordingly

        let mut last_synced_block = self.latest_block;

        let chain_tip = self
            .provider
            .get_block_number()
            .await
            .expect("TODO: handle error");

        while last_synced_block <= chain_tip {
            // NOTE: get all events by step
        }

        let discovery_manager = if let Some(factories) = self.factories {
            if self.discovery {
                Some(DiscoveryManager::new(factories))
            } else {
                None
            }
        } else {
            None
        };

        todo!();
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
        self.state.get_mut(&address)
    }
}
