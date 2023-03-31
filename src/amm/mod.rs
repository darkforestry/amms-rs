pub mod factory;
pub mod uniswap_v2;
pub mod uniswap_v3;

use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{H160, H256},
};
use serde::{Deserialize, Serialize};

use crate::errors::{ArithmeticError, DAMMError};

use self::{uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};

#[async_trait]
pub trait AutomatedMarketMaker {
    fn address(&self) -> H160;
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>>;
    fn sync_on_event_signatures(&self) -> Vec<H256>;
    fn tokens(&self) -> Vec<H160>;
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError>;
    async fn populate_data<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AMM {
    UniswapV2Pool(UniswapV2Pool),
    UniswapV3Pool(UniswapV3Pool),
}

#[async_trait]
impl AutomatedMarketMaker for AMM {
    fn address(&self) -> H160 {
        match self {
            AMM::UniswapV2Pool(pool) => pool.address,
            AMM::UniswapV3Pool(pool) => pool.address,
        }
    }

    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync(middleware).await,
            AMM::UniswapV3Pool(pool) => pool.sync(middleware).await,
        }
    }

    fn sync_on_event_signatures(&self) -> Vec<H256> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync_on_event_signatures(),
            AMM::UniswapV3Pool(pool) => pool.sync_on_event_signatures(),
        }
    }

    async fn populate_data<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.populate_data(middleware).await,
            AMM::UniswapV3Pool(pool) => pool.populate_data(middleware).await,
        }
    }

    fn tokens(&self) -> Vec<H160> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.tokens(),
            AMM::UniswapV3Pool(pool) => pool.tokens(),
        }
    }

    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.calculate_price(base_token),
            AMM::UniswapV3Pool(pool) => pool.calculate_price(base_token),
        }
    }
}
