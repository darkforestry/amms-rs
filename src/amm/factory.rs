use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{Log, H160, H256, U256},
};

use crate::errors::DAMMError;

use super::{uniswap_v2::factory::UniswapV2Factory, uniswap_v3::factory::UniswapV3Factory, AMM};

#[async_trait]
pub trait AutomatedMarketMakerFactory {
    fn address(&self) -> H160;

    async fn get_all_amms<M: Middleware>(
        &self,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>>;

    async fn populate_amms<M: Middleware>(
        &self,
        amms: &mut [AMM],
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>>;

    fn amm_created_event_signature(&self) -> H256;

    async fn new_amm_from_log<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, DAMMError<M>>;
}

#[derive(Clone, Copy)]
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

    async fn get_all_amms<M: Middleware>(
        &self,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.get_all_amms(middleware).await,
            Factory::UniswapV3Factory(factory) => factory.get_all_amms(middleware).await,
        }
    }

    async fn populate_amms<M: Middleware>(
        &self,
        amms: &mut [AMM],
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        match self {
            Factory::UniswapV2Factory(factory) => factory.populate_amms(amms, middleware).await,
            Factory::UniswapV3Factory(factory) => factory.populate_amms(amms, middleware).await,
        }
    }
}
