pub mod erc_4626;
pub mod factory;
pub mod uniswap_v2;
pub mod uniswap_v3;

use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{Log, H160, H256},
};
use serde::{Deserialize, Serialize};

use crate::errors::{ArithmeticError, DAMMError, EventLogError};

use self::{erc_4626::ERC4626Vault, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};

#[async_trait]
pub trait AutomatedMarketMaker {
    fn address(&self) -> H160;
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>>;
    fn sync_on_event_signatures(&self) -> Vec<H256>;
    fn tokens(&self) -> Vec<H160>;
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError>;
    fn sync_from_log(&mut self, log: &Log) -> Result<(), EventLogError>;
    async fn populate_data<M: Middleware>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AMM {
    UniswapV2Pool(UniswapV2Pool),
    UniswapV3Pool(UniswapV3Pool),
    ERC4626Vault(ERC4626Vault),
}

#[async_trait]
impl AutomatedMarketMaker for AMM {
    fn address(&self) -> H160 {
        match self {
            AMM::UniswapV2Pool(pool) => pool.address,
            AMM::UniswapV3Pool(pool) => pool.address,
            AMM::ERC4626Vault(vault) => vault.vault_token,
        }
    }

    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync(middleware).await,
            AMM::UniswapV3Pool(pool) => pool.sync(middleware).await,
            AMM::ERC4626Vault(vault) => vault.sync(middleware).await,
        }
    }

    fn sync_on_event_signatures(&self) -> Vec<H256> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync_on_event_signatures(),
            AMM::UniswapV3Pool(pool) => pool.sync_on_event_signatures(),
            AMM::ERC4626Vault(vault) => vault.sync_on_event_signatures(),
        }
    }

    fn sync_from_log(&mut self, log: &Log) -> Result<(), EventLogError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.sync_from_log(log),
            AMM::UniswapV3Pool(pool) => pool.sync_from_log(log),
            AMM::ERC4626Vault(vault) => vault.sync_from_log(log),
        }
    }

    async fn populate_data<M: Middleware>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.populate_data(None, middleware).await,
            AMM::UniswapV3Pool(pool) => pool.populate_data(block_number, middleware).await,
            AMM::ERC4626Vault(vault) => vault.populate_data(None, middleware).await,
        }
    }

    fn tokens(&self) -> Vec<H160> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.tokens(),
            AMM::UniswapV3Pool(pool) => pool.tokens(),
            AMM::ERC4626Vault(vault) => vault.tokens(),
        }
    }

    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        match self {
            AMM::UniswapV2Pool(pool) => pool.calculate_price(base_token),
            AMM::UniswapV3Pool(pool) => pool.calculate_price(base_token),
            AMM::ERC4626Vault(vault) => vault.calculate_price(base_token),
        }
    }
}
