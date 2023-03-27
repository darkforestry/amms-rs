use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{Log, H160, H256, U256},
};

use crate::{
    errors::{ArithmeticError, DAMMError},
    uniswap_v2::UniswapV2Pool,
};

#[async_trait]
pub trait AutomatedMarketMaker {
    fn address(&self) -> H160;
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>>;
    async fn sync_from_log(&mut self, log: &Log);
    fn sync_on_events(&self) -> Vec<H256>;
    fn tokens(&self) -> Vec<H160>;
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError>;
}

pub enum AMM {
    UniswapV2Pool(UniswapV2Pool),
    UniswapV3Pool(),
}
