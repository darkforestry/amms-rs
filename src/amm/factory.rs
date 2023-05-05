use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{BlockNumber, Filter, Log, ValueOrArray, H160, H256, U64},
};
use serde::{Deserialize, Serialize};

use crate::errors::DAMMError;

use super::{
    uniswap_v2::factory::{UniswapV2Factory, PAIR_CREATED_EVENT_SIGNATURE},
    uniswap_v3::factory::{UniswapV3Factory, POOL_CREATED_EVENT_SIGNATURE},
    AMM,
};

#[async_trait]
pub trait AutomatedMarketMakerFactory {
    fn address(&self) -> H160;

    async fn get_all_amms<M: Middleware>(
        &self,
        to_block: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>>;

    async fn populate_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>>;

    fn amm_created_event_signature(&self) -> H256;

    fn creation_block(&self) -> u64;

    async fn new_amm_from_log<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, DAMMError<M>>;

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Factory {
    UniswapV2Factory(UniswapV2Factory),
    UniswapV3Factory(UniswapV3Factory),
}

#[async_trait]
impl AutomatedMarketMakerFactory for Factory {
    fn address(&self) -> H160 {
        match self {
            Factory::UniswapV2Factory(factory) => factory.address(),
            Factory::UniswapV3Factory(factory) => factory.address(),
        }
    }

    fn amm_created_event_signature(&self) -> H256 {
        match self {
            Factory::UniswapV2Factory(factory) => factory.amm_created_event_signature(),
            Factory::UniswapV3Factory(factory) => factory.amm_created_event_signature(),
        }
    }

    async fn new_amm_from_log<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, DAMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.new_amm_from_log(log, middleware).await,
            Factory::UniswapV3Factory(factory) => factory.new_amm_from_log(log, middleware).await,
        }
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.new_empty_amm_from_log(log),
            Factory::UniswapV3Factory(factory) => factory.new_empty_amm_from_log(log),
        }
    }

    async fn get_all_amms<M: Middleware>(
        &self,
        to_block: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.get_all_amms(to_block, middleware).await,
            Factory::UniswapV3Factory(factory) => factory.get_all_amms(to_block, middleware).await,
        }
    }

    async fn populate_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => {
                factory.populate_amm_data(amms, None, middleware).await
            }
            Factory::UniswapV3Factory(factory) => {
                factory
                    .populate_amm_data(amms, block_number, middleware)
                    .await
            }
        }
    }

    fn creation_block(&self) -> u64 {
        match self {
            Factory::UniswapV2Factory(uniswap_v2_factory) => uniswap_v2_factory.creation_block,
            Factory::UniswapV3Factory(uniswap_v3_factory) => uniswap_v3_factory.creation_block,
        }
    }
}

impl Factory {
    pub async fn get_all_pools_from_logs<M: 'static + Middleware>(
        &self,
        mut from_block: u64,
        to_block: u64,
        step: u64,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        let mut aggregated_amms: Vec<AMM> = vec![];

        //TODO: ASYNC For each block within the range, get all pairs asynchronously

        while from_block < to_block {
            let provider = middleware.clone();

            let target_block = if from_block + step > to_block {
                to_block
            } else {
                from_block + step
            };

            let logs = provider
                .get_logs(
                    &Filter::new()
                        .topic0(ValueOrArray::Value(self.amm_created_event_signature()))
                        .address(self.address())
                        .from_block(BlockNumber::Number(U64([from_block])))
                        .to_block(BlockNumber::Number(U64([target_block]))),
                )
                .await
                .map_err(DAMMError::MiddlewareError)?;

            for log in logs {
                let amm = self.new_empty_amm_from_log(log)?;
                aggregated_amms.push(amm);
            }

            from_block = from_block + step;
        }

        Ok(aggregated_amms)
    }

    pub fn new_empty_factory_from_event_signature(event_signature: H256) -> Self {
        if event_signature == PAIR_CREATED_EVENT_SIGNATURE {
            Factory::UniswapV2Factory(UniswapV2Factory::default())
        } else if event_signature == POOL_CREATED_EVENT_SIGNATURE {
            Factory::UniswapV3Factory(UniswapV3Factory::default())
        } else {
            //TODO: handle this error
            panic!("Unrecognized event signature")
        }
    }
}
