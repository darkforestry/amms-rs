pub mod cache;
pub mod discovery;
pub mod filters;
pub mod tokens;

use crate::amms::amm::AutomatedMarketMaker;
use crate::amms::amm::AMM;
use crate::amms::factory::AutomatedMarketMakerFactory;
use crate::amms::factory::DiscoverySync;
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
use derive_more::derive::{Deref, DerefMut};
use discovery::DiscoveryManager;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use governor::Quota;
use governor::RateLimiter;
use std::collections::{BTreeMap, HashSet};
use std::num::NonZeroU32;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use tokens::populate_token_decimals;
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
    pub factories: Vec<Factory>,
    // NOTE: this is the list of filters each discovered pool will go through
    // pub filters: Vec<Filter>,
    pub discovery: bool,
    pub sync_step: u64,
    pub throttle: u32,
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
            discovery: false,
            sync_step: 10000,
            throttle: 0,
            phantom: PhantomData,
        }
    }

    pub fn block(self, latest_block: u64) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder {
            latest_block,
            ..self
        }
    }

    pub fn sync_step(self, sync_step: u64) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder { sync_step, ..self }
    }

    pub fn with_throttle(self, throttle: u32) -> StateSpaceBuilder<T, N, P> {
        StateSpaceBuilder { throttle, ..self }
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

    pub async fn sync(mut self) -> StateSpaceManager<T, N, P> {
        //NOTE: check if factories is empty

        // NOTE: Rather than using discmanager for sync, we can use it for filtering pools once running
        let discovery_manager = DiscoveryManager::new(self.factories.clone());

        // let throttle = if self.throttle > 0 {
        //     Some(Arc::new(RateLimiter::direct(Quota::per_second(
        //         NonZeroU32::new(self.throttle).unwrap(),
        //     ))))
        // } else {
        //     None
        // };

        let mut futures = FuturesUnordered::new();
        let factories = self.factories.clone();
        for factory in factories {
            let provider = self.provider.clone();

            // TODO: probably also need to specify latest block to sync to
            futures.push(tokio::spawn(async move {
                factory.discovery_sync(provider).await
            }));
        }

        while let Some(res) = futures.next().await {
            let amms = res.expect("TODO:").expect("TODO:");

            dbg!(&amms.len());
        }

        // TODO: handle amms from each task

        // // TODO: This might exceed max gas per static on some clients depending on the chain.
        // let token_decimals = populate_token_decimals(tokens, self.provider.clone())
        //     .await
        //     .expect("TODO: handle error");

        // for (_, amm) in state_space.iter_mut() {
        //     amm.set_decimals(&token_decimals);
        // }

        // dbg!(&state_space.len());

        // // TODO: filter amms with specified filters

        // StateSpaceManager {
        //     provider: self.provider,
        //     latest_block: self.latest_block,
        //     state: Arc::new(RwLock::new(state_space)),
        //     state_change_cache: Arc::new(RwLock::new(state_change_cache)),
        //     discovery_manager: Some(discovery_manager),
        //     block_filter,
        //     phantom: PhantomData,
        // }

        todo!();
    }
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct StateSpace(HashMap<Address, AMM>);
