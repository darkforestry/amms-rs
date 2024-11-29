use std::{
    future::Future,
    hash::{Hash, Hasher},
    sync::Arc,
};

use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::eth::Log,
    transports::Transport,
};
use eyre::Result;
use serde::{Deserialize, Serialize};

use super::{
    amm::{AutomatedMarketMaker, AMM},
    error::AMMError,
};

use super::uniswap_v2::UniswapV2Factory;
use super::uniswap_v3::UniswapV3Factory;

pub trait DiscoverySync {
    fn discover<T, N, P>(
        &self,
        to_block: u64,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>;

    fn sync<T, N, P>(
        amms: Vec<AMM>,
        to_block: u64,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>;
}

pub trait AutomatedMarketMakerFactory: DiscoverySync + Into<Factory> {
    type PoolVariant: AutomatedMarketMaker + Default;

    /// Returns the address of the factory.
    fn address(&self) -> Address;

    // TODO: update to be factory error?
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

        impl Factory {
             pub fn address(&self) -> Address {
                match self {
                    $(Factory::$factory_type(factory) => factory.address(),)+
                }
            }

             pub fn discovery_event(&self) -> B256 {
                match self {
                    $(Factory::$factory_type(factory) => factory.discovery_event(),)+
                }
            }

             pub fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
                match self {
                    $(Factory::$factory_type(factory) => factory.create_pool(log),)+
                }
            }

             pub fn creation_block(&self) -> u64 {
                match self {
                    $(Factory::$factory_type(factory) => factory.creation_block(),)+
                }
            }

             pub fn pool_events(&self) -> Vec<B256> {
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


        impl Factory {
            pub async fn discover<T, N, P>(&self, to_block: u64, provider: Arc<P>) -> Result<Vec<AMM>, AMMError>
            where
                T: Transport + Clone,
                N: Network,
                P: Provider<T, N>,
            {
                match self {
                    $(Factory::$factory_type(factory) => factory.discover(to_block, provider).await,)+
                }
            }

            pub async fn sync<T, N, P>(&self, amms: Vec<AMM>, to_block: u64, provider: Arc<P>) -> Result<Vec<AMM>, AMMError>
            where
                T: Transport + Clone,
                N: Network,
                P: Provider<T, N>,
            {
                match self {
                    $(Factory::$factory_type(factory) => factory.sync(amms, to_block, provider).await,)+
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

    fn sync(&mut self, _log: &Log) {
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
