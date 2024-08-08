use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

use alloy::{
    network::Network,
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::eth::{Filter, Log},
    sol_types::SolEvent,
    transports::Transport,
};
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};

use crate::errors::{AMMError, EventLogError};

use super::{
    uniswap_v2::factory::{IUniswapV2Factory, UniswapV2Factory},
    uniswap_v3::factory::{IUniswapV3Factory, UniswapV3Factory},
    AMM,
};

#[async_trait]
pub trait AutomatedMarketMakerFactory {
    /// Returns the address of the factory.
    fn address(&self) -> Address;

    /// Gets all Pools from the factory created logs up to the `to_block` block number.
    ///
    /// Returns a vector of AMMs.
    async fn get_all_amms<T, N, P>(
        &self,
        to_block: Option<u64>,
        provider: Arc<P>,
        step: u64,
    ) -> Result<Vec<AMM>, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>;

    /// Populates all AMMs data via batched static calls.
    async fn populate_amm_data<T, N, P>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        provider: Arc<P>,
    ) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>;

    /// Returns the creation event signature for the factory.
    fn amm_created_event_signature(&self) -> B256;

    /// Returns the block number at which the factory was created.
    fn creation_block(&self) -> u64;

    /// Creates a new AMM from a log factory creation event.
    ///
    /// Returns a AMM with data populated.
    async fn new_amm_from_log<T, N, P>(&self, log: Log, provider: Arc<P>) -> Result<AMM, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>;

    /// Creates a new empty AMM from a log factory creation event.
    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, alloy::sol_types::Error>;
}

macro_rules! factory {
    ($($factory_type:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Factory {
            $($factory_type($factory_type),)+
        }

        #[async_trait]
        impl AutomatedMarketMakerFactory for Factory {
            fn address(&self) -> Address {
                match self {
                    $(Factory::$factory_type(factory) => factory.address(),)+
                }
            }

            async fn get_all_amms<T, N, P>(
                &self,
                to_block: Option<u64>,
                provider: Arc<P>,
                step: u64,
            ) -> Result<Vec<AMM>, AMMError>
            where
                T: Transport + Clone,
                N: Network,
                P: Provider<T, N>,
            {
                match self {
                    $(Factory::$factory_type(factory) => {
                        factory.get_all_amms(to_block, provider, step).await
                    },)+
                }
            }

            async fn populate_amm_data<T, N, P>(
                &self,
                amms: &mut [AMM],
                block_number: Option<u64>,
                provider: Arc<P>,
            ) -> Result<(), AMMError>
            where
                T: Transport + Clone,
                N: Network,
                P: Provider<T, N>,
            {
                match self {
                    $(Factory::$factory_type(factory) => {
                        factory.populate_amm_data(amms, block_number, provider).await
                    },)+
                }
            }

            fn amm_created_event_signature(&self) -> B256 {
                match self {
                    $(Factory::$factory_type(factory) => factory.amm_created_event_signature(),)+
                }
            }

            fn creation_block(&self) -> u64 {
                match self {
                    $(Factory::$factory_type(factory) => factory.creation_block(),)+
                }
            }

            async fn new_amm_from_log<T, N, P>(
                &self,
                log: Log,
                provider: Arc<P>,
            ) -> Result<AMM, AMMError>
            where
                T: Transport + Clone,
                N: Network,
                P: Provider<T, N>,
            {
                match self {
                    $(Factory::$factory_type(factory) => factory.new_amm_from_log(log, provider).await,)+
                }
            }

            fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, alloy::sol_types::Error> {
                match self {
                    $(Factory::$factory_type(factory) => factory.new_empty_amm_from_log(log),)+
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

factory!(UniswapV2Factory, UniswapV3Factory);

impl Factory {
    pub async fn get_all_pools_from_logs<T, N, P>(
        &self,
        mut from_block: u64,
        to_block: u64,
        step: u64,
        provider: Arc<P>,
    ) -> Result<Vec<AMM>, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let factory_address = self.address();
        let amm_created_event_signature = self.amm_created_event_signature();
        let mut futures = FuturesUnordered::new();

        let mut aggregated_amms: Vec<AMM> = vec![];

        while from_block < to_block {
            let provider = provider.clone();
            let mut target_block = from_block + step - 1;
            if target_block > to_block {
                target_block = to_block;
            }

            let filter = Filter::new()
                .event_signature(amm_created_event_signature)
                .address(factory_address)
                .from_block(from_block)
                .to_block(target_block);

            futures.push(async move { provider.get_logs(&filter).await });

            from_block += step;
        }

        while let Some(result) = futures.next().await {
            let logs = result.map_err(AMMError::TransportError)?;

            for log in logs {
                aggregated_amms.push(self.new_empty_amm_from_log(log).unwrap());
            }
        }

        Ok(aggregated_amms)
    }
}

impl TryFrom<B256> for Factory {
    type Error = EventLogError;

    fn try_from(value: B256) -> Result<Self, Self::Error> {
        if value == IUniswapV2Factory::PairCreated::SIGNATURE_HASH {
            Ok(Factory::UniswapV2Factory(UniswapV2Factory::default()))
        } else if value == IUniswapV3Factory::PoolCreated::SIGNATURE_HASH {
            Ok(Factory::UniswapV3Factory(UniswapV3Factory::default()))
        } else {
            return Err(EventLogError::InvalidEventSignature);
        }
    }
}
