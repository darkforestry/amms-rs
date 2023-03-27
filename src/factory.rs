use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{Log, H160, H256, U256},
};

use crate::{
    errors::{ArithmeticError, DAMMError},
    uniswap_v2::UniswapV2Pool,
    uniswap_v3::UniswapV3Pool,
};

#[async_trait]
pub trait AMMFactory {
    fn address(&self) -> H160;
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>>;
    fn sync_on_event(&self) -> H256;
    fn tokens(&self) -> Vec<H160>;
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError>;
}

pub enum Factory {}
