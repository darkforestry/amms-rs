use std::{
    collections::HashMap,
    default,
    hash::{Hash, Hasher},
    sync::Arc,
};

use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::eth::{Filter, Log},
    sol_types::SolEvent,
    transports::Transport,
};
use eyre::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};

use super::{
    amm::{AutomatedMarketMaker, AMM},
    error::AMMError,
};

use super::uniswap_v2::UniswapV2Factory;
use super::uniswap_v3::UniswapV3Factory;

//TODO: add consts for steps, batch size, etc.

// TODO: DiscoverySync define how the factory will discover and sync initial pools upon initial sync
// pub trait AutomatedMarketMakerFactory: DiscoverySync
// NOTE: for uv2, discovery strategy is just call get all pairs, sync strat is to call get reserves on all pairs as a batch contract
// For some factories that need logs from a block range, you can configure a sync step upon factory initialization
// pub trait DiscoverySync {
//     fn discovery_sync(&self, provider) -> Vec<AMM>;
//}

pub trait DiscoverySync {
    fn discovery_sync<T, N, P>(&self, provider: Arc<P>) -> Vec<AMM>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>;
}

pub trait AutomatedMarketMakerFactory: DiscoverySync + Into<Factory> {
    type PoolVariant: AutomatedMarketMaker + Default;

    /// Returns the address of the factory.
    fn address(&self) -> Address;

    // TODO: update to be factory error
    fn create_pool(&self, log: Log) -> Result<AMM, AMMError>;

    /// Returns the block number at which the factory was created.
    fn creation_block(&self) -> u64;

    fn discovery_event(&self) -> B256;

    fn pool_events(&self) -> Vec<B256> {
        Self::PoolVariant::default().sync_events()
    }
}

macro_rules! factory {
    ($($factory_type:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Factory {
            $($factory_type($factory_type),)+
        }

        impl AutomatedMarketMakerFactory for Factory {
            type PoolVariant = NoopAMM;
             fn address(&self) -> Address {
                match self {
                    $(Factory::$factory_type(factory) => factory.address(),)+
                }
            }

             fn discovery_event(&self) -> B256 {
                match self {
                    $(Factory::$factory_type(factory) => factory.discovery_event(),)+
                }
            }

             fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
                match self {
                    $(Factory::$factory_type(factory) => factory.create_pool(log),)+
                }
            }

             fn creation_block(&self) -> u64 {
                match self {
                    $(Factory::$factory_type(factory) => factory.creation_block(),)+
                }
            }

             fn pool_events(&self) -> Vec<B256> {
                match self {
                    $(Factory::$factory_type(factory) => factory.pool_events(),)+
                }
            }
        }

        impl Hash for Factory {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.address().hash(state);
            }
        }

        impl PartialEq for Factory {
            fn eq(&self, other: &Self) -> bool {
                self.address() == other.address()
            }
        }

        impl Eq for Factory {}


        impl DiscoverySync for Factory {
            fn discovery_sync<T, N, P>(&self, provider: Arc<P>) -> Vec<AMM>
            where
                T: Transport + Clone,
                N: Network,
                P: Provider<T, N>,
            {
                match self {
                    $(Factory::$factory_type(factory) => factory.discovery_sync(provider),)+
                }
            }
        }
    };
}

factory!(UniswapV2Factory, UniswapV3Factory);

#[derive(Default)]
pub struct NoopAMM;
impl AutomatedMarketMaker for NoopAMM {
    fn address(&self) -> Address {
        unreachable!()
    }

    fn sync_events(&self) -> Vec<B256> {
        unreachable!()
    }

    fn sync(&mut self, _log: Log) {
        unreachable!()
    }

    fn set_decimals(&mut self, _token_decimals: &HashMap<Address, u8>) {
        unreachable!()
    }

    fn simulate_swap(
        &self,
        _base_token: Address,
        _quote_token: Address,
        _amount_in: U256,
    ) -> Result<U256, AMMError> {
        unreachable!()
    }

    fn simulate_swap_mut(
        &mut self,
        _base_token: Address,
        _quote_token: Address,
        _amount_in: U256,
    ) -> Result<U256, AMMError> {
        unreachable!()
    }
    fn calculate_price(
        &self,
        _base_token: Address,
        _quote_token: Address,
    ) -> Result<f64, AMMError> {
        unreachable!()
    }

    fn tokens(&self) -> Vec<Address> {
        unreachable!()
    }
}
