use super::{
    amm::{AutomatedMarketMaker, AMM},
    error::AMMError,
    factory::{AutomatedMarketMakerFactory, Factory},
};

use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::{Filter, Log},
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

sol!(
// UniswapV2Factory
#[allow(missing_docs)]
#[derive(Debug)]
contract IUniswapV3Factory {
}

#[derive(Debug, PartialEq, Eq)]
#[sol(rpc)]
contract IUniswapV3Pool {
    event Sync(uint112 reserve0, uint112 reserve1);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data);
}
);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV3Pool {
    pub address: Address,
    pub token_a: Address,
    pub token_a_decimals: u8,
    pub token_b: Address,
    pub token_b_decimals: u8,
    pub fee: usize,
}

impl AutomatedMarketMaker for UniswapV3Pool {
    fn address(&self) -> Address {
        self.address
    }

    fn sync_events(&self) -> Vec<B256> {
        todo!()
    }

    fn set_decimals(&mut self, token_decimals: &HashMap<Address, u8>) {
        todo!()
    }

    fn sync(&mut self, log: Log) {
        todo!()
    }

    fn simulate_swap(
        &self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        todo!()
    }

    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        todo!()
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.token_a, self.token_b]
    }

    fn calculate_price(&self, base_token: Address, quote_token: Address) -> Result<f64, AMMError> {
        todo!()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct UniswapV3Factory {
    pub address: Address,
    pub fee: usize,
    pub creation_block: u64,
}

impl UniswapV3Factory {
    pub fn new(address: Address, fee: usize, creation_block: u64) -> Self {
        todo!()
    }
}

impl Into<Factory> for UniswapV3Factory {
    fn into(self) -> Factory {
        Factory::UniswapV3Factory(self)
    }
}

impl AutomatedMarketMakerFactory for UniswapV3Factory {
    type PoolVariant = UniswapV3Pool;

    fn address(&self) -> Address {
        self.address
    }

    fn discovery_events(&self) -> Vec<B256> {
        todo!()
    }

    fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
        todo!()
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }
}
