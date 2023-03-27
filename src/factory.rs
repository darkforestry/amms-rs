use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{Log, H160, H256, U256},
};

use crate::{
    amm::AMM,
    errors::{ArithmeticError, DAMMError},
    uniswap_v2::UniswapV2Pool,
    uniswap_v3::UniswapV3Pool,
};

#[async_trait]
pub trait AutomatedMarketMakerFactory {
    fn address(&self) -> H160;
    async fn get_all_amms<M: Middleware>(
        &self,
        step: usize,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>>;
}

#[derive(Clone, Copy)]
pub enum Factory {
    UniswapV2Factory(),
    UniswapV3Factory(),
}

#[async_trait]
impl AutomatedMarketMakerFactory for Factory {
    fn address(&self) -> H160 {
        todo!()
    }

    async fn get_all_amms<M: Middleware>(
        &self,
        step: usize,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        todo!()
    }
}
