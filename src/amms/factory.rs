use std::{
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
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};

use super::{
    amm::{AutomatedMarketMaker, AMM},
    error::AMMError,
};

use super::uniswap_v2::UniswapV2Factory;

//TODO: add consts for steps, batch size, etc.
pub trait AutomatedMarketMakerFactory: Into<Factory> {
    //TODO: GAT for AMM
    type PoolVariant: AutomatedMarketMaker + Default;

    /// Returns the address of the factory.
    fn address(&self) -> Address;

    // TODO: update to be factory error
    fn create_pool(&self, log: Log) -> Result<AMM, AMMError>;

    /// Returns the block number at which the factory was created.
    fn creation_block(&self) -> u64;

    fn discovery_events(&self) -> Vec<B256>;

    fn pool_events(&self) -> Vec<B256> {
        Self::PoolVariant::default().sync_events()
    }

    // TODO: new_pool (empty pool from log), need to think through the best way to get decimals
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

            fn discovery_events(&self) -> Vec<B256> {
                match self {
                    $(Factory::$factory_type(factory) => factory.discovery_events(),)+
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
    };
}

factory!(UniswapV2Factory);

#[derive(Default)]
struct NoopAMM;
impl AutomatedMarketMaker for NoopAMM {
    fn address(&self) -> Address {
        unreachable!()
    }

    fn sync_events(&self) -> Vec<B256> {
        unreachable!()
    }

    fn sync(&mut self, log: Log) {
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
