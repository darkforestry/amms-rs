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
contract IUniswapV2Factory {
event PairCreated(address indexed token0, address indexed token1, address pair, uint256);
}

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

    fn set_decimals(&mut self, token_decimals: &HashMap<Address, u8>) {
        self.token_a_decimals = *token_decimals.get(&self.token_a).expect("TODO:");
        self.token_b_decimals = *token_decimals.get(&self.token_b).expect("TODO:");
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
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if self.token_a == base_token {
            Ok(self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            ))
        } else {
            Ok(self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            ))
        }
    }

    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if self.token_a == base_token {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            );

            self.reserve_0 += amount_in.to::<u128>();
            self.reserve_1 -= amount_out.to::<u128>();

            Ok(amount_out)
        } else {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            );

            self.reserve_0 -= amount_out.to::<u128>();
            self.reserve_1 += amount_in.to::<u128>();

            Ok(amount_out)
        }
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.token_a, self.token_b]
    }

    fn calculate_price(&self, base_token: Address, quote_token: Address) -> Result<f64, AMMError> {
        todo!()
    }
}

impl UniswapV2Pool {
    /// Calculates the amount received for a given `amount_in` `reserve_in` and `reserve_out`.
    pub fn get_amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }

        // TODO: we could set this as the fee on the pool instead of calculating this
        let fee = (10000 - (self.fee / 10)) / 10; //Fee of 300 => (10,000 - 30) / 10  = 997
        let amount_in_with_fee = amount_in * U256::from(fee);
        let numerator = amount_in_with_fee * reserve_out;
        // TODO: update U256::from(1000) to const
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;

        numerator / denominator
    }
}

impl Into<AMM> for UniswapV2Pool {
    fn into(self) -> AMM {
        AMM::UniswapV2Pool(self)
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

    fn discovery_event(&self) -> B256 {
        IUniswapV2Factory::PairCreated::SIGNATURE_HASH
    }

    fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
        let event = IUniswapV2Factory::PairCreated::decode_log(&log.inner, false)
            .expect("TODO: handle this error");
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
