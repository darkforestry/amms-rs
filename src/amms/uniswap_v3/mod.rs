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
    /// @notice Emitted when a pool is created
    event PoolCreated(
        address indexed token0,
        address indexed token1,
        uint24 indexed fee,
        int24 tickSpacing,
        address pool
    );
}

#[derive(Debug, PartialEq, Eq)]
#[sol(rpc)]
contract IUniswapV3PoolEvents {
    /// @notice Emitted when liquidity is minted for a given position
    event Mint(
        address sender,
        address indexed owner,
        int24 indexed tickLower,
        int24 indexed tickUpper,
        uint128 amount,
        uint256 amount0,
        uint256 amount1
    );

    /// @notice Emitted when a position's liquidity is removed
    event Burn(
        address indexed owner,
        int24 indexed tickLower,
        int24 indexed tickUpper,
        uint128 amount,
        uint256 amount0,
        uint256 amount1
    );

    /// @notice Emitted by the pool for any swaps between token0 and token1
    event Swap(
        address indexed sender,
        address indexed recipient,
        int256 amount0,
        int256 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick
    );

});

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV3Pool {
    pub address: Address,
    pub token_a: Address,
    pub token_a_decimals: u8,
    pub token_b: Address,
    pub token_b_decimals: u8,
    pub liquidity: u128,
    pub sqrt_price: U256,
    pub fee: u32,
    pub tick: i32,
    pub tick_spacing: i32,
    pub tick_bitmap: HashMap<i16, U256>,
    pub ticks: HashMap<i32, Info>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Info {
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
    pub initialized: bool,
}

impl Info {
    pub fn new(liquidity_gross: u128, liquidity_net: i128, initialized: bool) -> Self {
        Info {
            liquidity_gross,
            liquidity_net,
            initialized,
        }
    }
}
impl AutomatedMarketMaker for UniswapV3Pool {
    fn address(&self) -> Address {
        self.address
    }

    fn sync_events(&self) -> Vec<B256> {
        vec![
            IUniswapV3PoolEvents::Mint::SIGNATURE_HASH,
            IUniswapV3PoolEvents::Burn::SIGNATURE_HASH,
            IUniswapV3PoolEvents::Swap::SIGNATURE_HASH,
        ]
    }

    fn set_decimals(&mut self, token_decimals: &HashMap<Address, u8>) {
        self.token_a_decimals = *token_decimals.get(&self.token_a).expect("TODO:");
        self.token_b_decimals = *token_decimals.get(&self.token_b).expect("TODO:");
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
    pub creation_block: u64,
}

impl UniswapV3Factory {
    pub fn new(address: Address, creation_block: u64) -> Self {
        UniswapV3Factory {
            address,
            creation_block,
        }
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

    fn discovery_event(&self) -> B256 {
        IUniswapV3Factory::PoolCreated::SIGNATURE_HASH
    }

    fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
        let pool_created_event =
            IUniswapV3Factory::PoolCreated::decode_log(&log.inner, true).expect("TODO:");

        Ok(AMM::UniswapV3Pool(UniswapV3Pool {
            address: pool_created_event.pool,
            token_a: pool_created_event.token0,
            token_b: pool_created_event.token1,
            fee: pool_created_event.fee.to::<u32>(),
            tick_spacing: pool_created_event.tickSpacing.unchecked_into(),
            ..Default::default()
        }))
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }
}
