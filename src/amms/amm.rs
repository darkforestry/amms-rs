use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::Log,
    transports::Transport,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    hash::{Hash, Hasher},
    sync::Arc,
};

use super::{error::AMMError, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool};

pub trait AutomatedMarketMaker {
    // TODO: maybe add a sync step and batch size GAT that will be implemented for each amm

    /// Returns the address of the AMM.
    fn address(&self) -> Address;

    fn sync_events(&self) -> Vec<B256>;

    fn set_decimals(&mut self, token_decimals: &HashMap<Address, u8>);

    fn sync(&mut self, log: Log);

    /// Returns a vector of tokens in the AMM.
    fn tokens(&self) -> Vec<Address>;

    /// Calculates a f64 representation of base token price in the AMM.
    fn calculate_price(&self, base_token: Address, quote_token: Address) -> Result<f64, AMMError>;

    /// Locally simulates a swap in the AMM.
    /// Returns the amount received for `amount_in` of `token_in`.
    fn simulate_swap(
        &self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError>;

    /// Locally simulates a swap in the AMM.
    /// Mutates the AMM state to the state of the AMM after swapping.
    /// Returns the amount received for `amount_in` of `token_in`.
    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError>;
}

macro_rules! amm {
    ($($pool_type:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum AMM {
            $($pool_type($pool_type),)+
        }

        impl AutomatedMarketMaker for AMM {
            fn address(&self) -> Address{
                match self {
                    $(AMM::$pool_type(pool) => pool.address(),)+
                }
            }

            fn sync_events(&self) -> Vec<B256> {
                match self {
                    $(AMM::$pool_type(pool) => pool.sync_events(),)+
                }
            }

            fn set_decimals(&mut self, token_decimals: &HashMap<Address, u8>) {
                match self {
                    $(AMM::$pool_type(pool) => pool.set_decimals(token_decimals),)+
                }
            }

            fn sync(&mut self, log: Log) {
                match self {
                    $(AMM::$pool_type(pool) => pool.sync(log),)+
                }
            }

            fn simulate_swap(&self, base_token: Address, quote_token: Address,amount_in: U256) -> Result<U256, AMMError> {
                match self {
                    $(AMM::$pool_type(pool) => pool.simulate_swap(base_token, quote_token, amount_in),)+
                }
            }

            fn simulate_swap_mut(&mut self, base_token: Address, quote_token: Address, amount_in: U256) -> Result<U256, AMMError> {
                match self {
                    $(AMM::$pool_type(pool) => pool.simulate_swap_mut(base_token, quote_token, amount_in),)+
                }
            }

            fn tokens(&self) -> Vec<Address> {
                match self {
                    $(AMM::$pool_type(pool) => pool.tokens(),)+
                }
            }

            fn calculate_price(&self, base_token: Address, quote_token: Address) -> Result<f64, AMMError> {
                match self {
                    $(AMM::$pool_type(pool) => pool.calculate_price(base_token, quote_token),)+
                }
            }
        }

        impl Hash for AMM {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.address().hash(state);
            }
        }

        impl PartialEq for AMM {
            fn eq(&self, other: &Self) -> bool {
                self.address() == other.address()
            }
        }

        impl Eq for AMM {}
    };
}

amm!(UniswapV2Pool, UniswapV3Pool);
