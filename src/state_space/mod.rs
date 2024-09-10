pub mod cache;
pub mod discovery;
pub mod filters;

use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use alloy::{network::Network, primitives::Address, providers::Provider, transports::Transport};
use cache::StateChangeCache;
use discovery::DiscoveryManager;
use tokio::sync::RwLock;

use crate::amms::{amm::AMM, factory::Factory};

pub const CACHE_SIZE: usize = 30;

pub struct StateSpaceManager<T, N, P> {
    pub provider: Arc<P>,
    pub factories: Vec<Factory>,
    pub state: Arc<RwLock<StateSpace>>,
    // NOTE: explore more efficient rw locks
    state_change_cache: Arc<RwLock<StateChangeCache<CACHE_SIZE>>>,
    // NOTE: does this need to be atomic u64?
    latest_block: u64,
    discovery_manager: DiscoveryManager,
    // TODO: add support for caching
    phantom: PhantomData<(T, N)>,
}

#[derive(Debug, Default)]
pub struct StateSpaceBuilder<T, N, P> {
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
    P: Provider<T, N> + 'static,
{
    pub fn new(provider: Arc<P>) -> StateSpaceBuilder<T, N, P> {
        Self {
            provider,
            latest_block: 0,
            factories: vec![],
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
        StateSpaceBuilder { factories, ..self }
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
        StateSpaceManager {
            provider: self.provider,
            latest_block: self.latest_block,
            state: Arc::new(RwLock::new(StateSpace::default())),
            state_change_cache: Arc::new(RwLock::new(StateChangeCache::new())),
            factories: self.factories.clone(),
            discovery_manager: DiscoveryManager::new(self.factories),
            phantom: PhantomData,
        }
    }
}

#[derive(Debug, Default)]
pub struct StateSpace {
    // NOTE: bench dashmap instead
    state: HashMap<Address, AMM>,
}
