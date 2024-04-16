use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::{Middleware, StreamExt},
    types::{BlockNumber, Filter, Log, ValueOrArray, H160, H256, U64},
};
use futures::stream::FuturesUnordered;
use serde::{Deserialize, Serialize};

use crate::errors::{AMMError, EventLogError};

use super::{
    uniswap_v2::factory::{UniswapV2Factory, PAIR_CREATED_EVENT_SIGNATURE},
    uniswap_v3::factory::{UniswapV3Factory, POOL_CREATED_EVENT_SIGNATURE},
    AMM,
};

#[async_trait]
pub trait AutomatedMarketMakerFactory {
    /// Returns the address of the factory.
    fn address(&self) -> H160;

    /// Gets all Pools from the factory created logs up to the `to_block` block number.
    ///
    /// Returns a vector of AMMs.
    async fn get_all_amms<M: 'static + Middleware>(
        &self,
        to_block: Option<u64>,
        middleware: Arc<M>,
        step: u64,
    ) -> Result<Vec<AMM>, AMMError<M>>;

    /// Populates all AMMs data via batched static calls.
    async fn populate_amm_data<M: 'static + Middleware>(
        &self,
        amms: &mut [AMM],
        from_block: Option<u64>,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AMMError<M>>;

    /// Returns the creation event signature for the factory.
    fn amm_created_event_signature(&self) -> H256;

    /// Returns the block number at which the factory was created.
    fn creation_block(&self) -> u64;

    /// Creates a new AMM from a log factory creation event.
    ///
    /// Returns a AMM with data populated.
    async fn new_amm_from_log<M: 'static + Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, AMMError<M>>;

    /// Creates a new empty AMM from a log factory creation event.
    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error>;
}

macro_rules! factory {
    ($($factory_type:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Factory {
            $($factory_type($factory_type),)+
        }

        #[async_trait]
        impl AutomatedMarketMakerFactory for Factory {
            fn address(&self) -> H160 {
                match self {
                    $(Factory::$factory_type(factory) => factory.address(),)+
                }
            }

            async fn get_all_amms<M: 'static + Middleware>(
                &self,
                to_block: Option<u64>,
                middleware: Arc<M>,
                step: u64,
            ) -> Result<Vec<AMM>, AMMError<M>> {
                match self {
                    $(Factory::$factory_type(factory) => {
                        factory.get_all_amms(to_block, middleware, step).await
                    },)+
                }
            }

            async fn populate_amm_data<M: 'static + Middleware>(
                &self,
                amms: &mut [AMM],
                from_block: Option<u64>,
                block_number: Option<u64>,
                middleware: Arc<M>,
            ) -> Result<(), AMMError<M>> {
                match self {
                    $(Factory::$factory_type(factory) => {
                        factory.populate_amm_data(amms, from_block, block_number, middleware).await
                    },)+
                }
            }

            fn amm_created_event_signature(&self) -> H256 {
                match self {
                    $(Factory::$factory_type(factory) => factory.amm_created_event_signature(),)+
                }
            }

            fn creation_block(&self) -> u64 {
                match self {
                    $(Factory::$factory_type(factory) => factory.creation_block(),)+
                }
            }

            async fn new_amm_from_log<M: 'static + Middleware>(
                &self,
                log: Log,
                middleware: Arc<M>,
            ) -> Result<AMM, AMMError<M>> {
                match self {
                    $(Factory::$factory_type(factory) => factory.new_amm_from_log(log, middleware).await,)+
                }
            }

            fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error> {
                match self {
                    $(Factory::$factory_type(factory) => factory.new_empty_amm_from_log(log),)+
                }
            }
        }
    };
}

factory!(UniswapV2Factory, UniswapV3Factory);

impl Factory {
    pub async fn get_all_pools_from_logs<M: 'static + Middleware>(
        &self,
        mut from_block: u64,
        to_block: u64,
        step: u64,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, AMMError<M>> {
        let factory_address = self.address();
        let amm_created_event_signature = self.amm_created_event_signature();
        let mut futures = FuturesUnordered::new();

        let mut aggregated_amms: Vec<AMM> = vec![];

        while from_block < to_block {
            let middleware = middleware.clone();
            let mut target_block = from_block + step - 1;
            if target_block > to_block {
                target_block = to_block;
            }

            let filter = Filter::new()
                .topic0(ValueOrArray::Value(amm_created_event_signature))
                .address(factory_address)
                .from_block(BlockNumber::Number(U64([from_block])))
                .to_block(BlockNumber::Number(U64([target_block])));

            futures.push(async move { middleware.get_logs(&filter).await });

            from_block += step;
        }

        while let Some(result) = futures.next().await {
            let logs = result.map_err(AMMError::MiddlewareError)?;

            for log in logs {
                aggregated_amms.push(self.new_empty_amm_from_log(log)?);
            }
        }

        Ok(aggregated_amms)
    }
}

impl TryFrom<H256> for Factory {
    type Error = EventLogError;

    fn try_from(value: H256) -> Result<Self, Self::Error> {
        if value == PAIR_CREATED_EVENT_SIGNATURE {
            Ok(Factory::UniswapV2Factory(UniswapV2Factory::default()))
        } else if value == POOL_CREATED_EVENT_SIGNATURE {
            Ok(Factory::UniswapV3Factory(UniswapV3Factory::default()))
        } else {
            return Err(EventLogError::InvalidEventSignature);
        }
    }
}
