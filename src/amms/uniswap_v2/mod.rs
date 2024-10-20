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
    hash::{Hash, Hasher},
    sync::Arc,
};

sol!(
// UniswapV2Factory
#[allow(missing_docs)]
#[derive(Debug)]
event PairCreated(address indexed token0, address indexed token1, address pair, uint256);

#[derive(Debug, PartialEq, Eq)]
#[sol(rpc)]
contract IUniswapV2Pair {
    event Sync(uint112 reserve0, uint112 reserve1);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data);
});

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV2Pool {
    pub address: Address,
    pub token_a: Address,
    pub token_a_decimals: u8,
    pub token_b: Address,
    pub token_b_decimals: u8,
    pub reserve_0: u128,
    pub reserve_1: u128,
    pub fee: usize,
}

impl AutomatedMarketMaker for UniswapV2Pool {
    fn address(&self) -> Address {
        self.address
    }

    fn sync_events(&self) -> Vec<B256> {
        vec![IUniswapV2Pair::Sync::SIGNATURE_HASH]
    }

    fn set_decimals(&mut self, token_decimals: &std::collections::HashMap<Address, u8>) {
        self.token_a_decimals = *token_decimals.get(&self.token_a).unwrap();
        self.token_b_decimals = *token_decimals.get(&self.token_b).unwrap();
    }

    fn sync(&mut self, log: Log) {
        let sync_event =
            IUniswapV2Pair::Sync::decode_log(&log.inner, false).expect("TODO: handle this error");

        let (reserve_0, reserve_1) = (
            sync_event.reserve0.to::<u128>(),
            sync_event.reserve1.to::<u128>(),
        );
        // tracing::info!(reserve_0, reserve_1, address = ?self.address, "UniswapV2 sync event");

        self.reserve_0 = reserve_0;
        self.reserve_1 = reserve_1;
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
pub struct UniswapV2Factory {
    pub address: Address,
    pub fee: usize,
    pub creation_block: u64,
}

impl UniswapV2Factory {
    pub fn new(address: Address, fee: usize, creation_block: u64) -> Self {
        Self {
            address,
            creation_block,
            fee,
        }
    }
}

impl Into<Factory> for UniswapV2Factory {
    fn into(self) -> Factory {
        Factory::UniswapV2Factory(self)
    }
}

impl AutomatedMarketMakerFactory for UniswapV2Factory {
    type PoolVariant = UniswapV2Pool;

    fn address(&self) -> Address {
        self.address
    }

    fn discovery_events(&self) -> Vec<B256> {
        todo!()
    }

    fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
        let event = PairCreated::decode_log(&log.inner, false).expect("TODO: handle this error");
        Ok(AMM::UniswapV2Pool(UniswapV2Pool {
            address: event.pair,
            token_a: event.token0,
            token_a_decimals: 0,
            token_b: event.token1,
            token_b_decimals: 0,
            reserve_0: 0,
            reserve_1: 0,
            fee: self.fee,
        }))
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }
}
