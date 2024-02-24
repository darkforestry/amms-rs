pub mod erc_4626;
pub mod factory;
pub mod uniswap_v2;
pub mod uniswap_v3;

use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{Log, H160, H256, U256},
};
use serde::{Deserialize, Serialize};

use crate::errors::{AMMError, ArithmeticError, EventLogError, SwapSimulationError};

use self::{erc_4626::ERC4626Vault, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};

#[async_trait]
pub trait AutomatedMarketMaker {
    /// Returns the address of the AMM.
    fn address(&self) -> H160;

    /// Syncs the AMM data on chain via batched static calls.
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), AMMError<M>>;

    /// Returns the vector of event signatures subscribed to when syncing the AMM.
    fn sync_on_event_signatures(&self) -> Vec<H256>;

    /// Returns a vector of tokens in the AMM.
    fn tokens(&self) -> Vec<H160>;

    /// Calculates a f64 representation of base token price in the AMM.
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError>;

    /// Updates the AMM data from a log.
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError>;

    /// Populates the AMM data via batched static calls.
    async fn populate_data<M: Middleware>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AMMError<M>>;

    /// Locally simulates a swap in the AMM.
    ///
    /// Returns the amount received for `amount_in` of `token_in`.
    fn simulate_swap(&self, token_in: H160, amount_in: U256) -> Result<U256, SwapSimulationError>;

    /// Locally simulates a swap in the AMM.
    /// Mutates the AMM state to the state of the AMM after swapping.
    /// Returns the amount received for `amount_in` of `token_in`.
    fn simulate_swap_mut(
        &mut self,
        token_in: H160,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError>;

    /// Returns the token out of the AMM for a given `token_in`.
    fn get_token_out(&self, token_in: H160) -> H160;
}

macro_rules! amm {
    ($($pool_type:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum AMM {
            $($pool_type($pool_type),)+
        }

        #[async_trait]
        impl AutomatedMarketMaker for AMM {
            fn address(&self) -> H160 {
                match self {
                    $(AMM::$pool_type(pool) => pool.address(),)+
                }
            }

            async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), AMMError<M>> {
                match self {
                    $(AMM::$pool_type(pool) => pool.sync(middleware).await,)+
                }
            }

            fn sync_on_event_signatures(&self) -> Vec<H256> {
                match self {
                    $(AMM::$pool_type(pool) => pool.sync_on_event_signatures(),)+
                }
            }

            fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
                match self {
                    $(AMM::$pool_type(pool) => pool.sync_from_log(log),)+
                }
            }

            fn simulate_swap(&self, token_in: H160, amount_in: U256) -> Result<U256, SwapSimulationError> {
                match self {
                    $(AMM::$pool_type(pool) => pool.simulate_swap(token_in, amount_in),)+
                }
            }

            fn simulate_swap_mut(&mut self, token_in: H160, amount_in: U256) -> Result<U256, SwapSimulationError> {
                match self {
                    $(AMM::$pool_type(pool) => pool.simulate_swap_mut(token_in, amount_in),)+
                }
            }

            fn get_token_out(&self, token_in: H160) -> H160 {
                match self {
                    $(AMM::$pool_type(pool) => pool.get_token_out(token_in),)+
                }
            }

            async fn populate_data<M: Middleware>(&mut self, block_number: Option<u64>, middleware: Arc<M>) -> Result<(), AMMError<M>> {
                match self {
                    $(AMM::$pool_type(pool) => pool.populate_data(block_number, middleware).await,)+
                }
            }

            fn tokens(&self) -> Vec<H160> {
                match self {
                    $(AMM::$pool_type(pool) => pool.tokens(),)+
                }
            }

            fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
                match self {
                    $(AMM::$pool_type(pool) => pool.calculate_price(base_token),)+
                }
            }
        }
    };
}

amm!(UniswapV2Pool, UniswapV3Pool, ERC4626Vault);
