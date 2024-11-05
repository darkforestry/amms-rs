use crate::{
    amms::{consts::U256_1, uniswap_v2::q64_to_float},
    state_space::StateSpace,
};

use super::{
    amm::{AutomatedMarketMaker, AMM},
    error::AMMError,
    factory::{AutomatedMarketMakerFactory, DiscoverySync, Factory},
    get_token_decimals,
};

use crate::amms::uniswap_v3::GetUniswapV3PoolTickBitmapBatchRequest::TickBitmapInfo;
use alloy::{
    dyn_abi::DynSolType,
    network::Network,
    primitives::{address, Address, Signed, B256, I256, U256},
    providers::Provider,
    rpc::types::{Filter, FilterSet, Log},
    signers::k256::elliptic_curve::{group, rand_core::le},
    sol,
    sol_types::{SolEvent, SolValue},
    transports::Transport,
};
use eyre::Result;
use futures::{
    stream::{FuturesOrdered, FuturesUnordered},
    StreamExt,
};
use governor::{Quota, RateLimiter};
use itertools::Itertools;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
    hash::Hash,
    num::NonZeroU32,
    result,
    str::FromStr,
    sync::Arc,
    time,
};
use uniswap_v3_math::{
    tick,
    tick_bitmap::{self, next_initialized_tick_within_one_word},
    tick_math::{MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK},
};
use GetUniswapV3PoolTickDataBatchRequest::{
    GetUniswapV3PoolTickDataBatchRequestInstance, TickDataInfo,
};

sol! {
    // UniswapV3Factory
    #[allow(missing_docs)]
    #[derive(Debug)]
    #[sol(rpc)]
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
    }
}

sol! {
#[sol(rpc)]
GetUniswapV3PoolSlot0BatchRequest,
"contracts/out/GetUniswapV3PoolSlot0BatchRequest.sol/GetUniswapV3PoolSlot0BatchRequest.json",
}

sol! {
#[sol(rpc)]
GetUniswapV3PoolTickBitmapBatchRequest,
"contracts/out/GetUniswapV3PoolTickBitmapBatchRequest.sol/GetUniswapV3PoolTickBitmapBatchRequest.json",
}

sol! {
#[sol(rpc)]
GetUniswapV3PoolTickDataBatchRequest,
"contracts/out/GetUniswapV3PoolTickDataBatchRequest.sol/GetUniswapV3PoolTickDataBatchRequest.json"
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV3Pool {
    pub address: Address,
    pub token_a: Address,
    pub token_a_decimals: u8, // NOTE:
    pub token_b: Address,
    pub token_b_decimals: u8, // NOTE:
    pub liquidity: u128,      // NOTE:
    pub sqrt_price: U256,     // NOTE:
    pub fee: u32,
    pub tick: i32, // NOTE:
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

pub struct CurrentState {
    amount_specified_remaining: I256,
    amount_calculated: I256,
    sqrt_price_x_96: U256,
    tick: i32,
    liquidity: u128,
}

#[derive(Default)]
pub struct StepComputations {
    pub sqrt_price_start_x_96: U256,
    pub tick_next: i32,
    pub initialized: bool,
    pub sqrt_price_next_x96: U256,
    pub amount_in: U256,
    pub amount_out: U256,
    pub fee_amount: U256,
}

pub struct Tick {
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
    pub fee_growth_outside_0_x_128: U256,
    pub fee_growth_outside_1_x_128: U256,
    pub tick_cumulative_outside: U256,
    pub seconds_per_liquidity_outside_x_128: U256,
    pub seconds_outside: u32,
    pub initialized: bool,
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

    fn sync(&mut self, log: Log) {
        let event_signature = log.topics()[0];
        match event_signature {
            IUniswapV3PoolEvents::Swap::SIGNATURE_HASH => {
                let swap_event =
                    IUniswapV3PoolEvents::Swap::decode_log(log.as_ref(), false).expect("TODO: ");

                self.sqrt_price = swap_event.sqrtPriceX96.to();
                self.liquidity = swap_event.liquidity;
                self.tick = swap_event.tick.unchecked_into();

                // tracing::debug!(?swap_event, address = ?self.address, sqrt_price = ?self.sqrt_price, liquidity = ?self.liquidity, tick = ?self.tick, "UniswapV3 swap event");
            }
            IUniswapV3PoolEvents::Mint::SIGNATURE_HASH => {
                let mint_event =
                    IUniswapV3PoolEvents::Mint::decode_log(log.as_ref(), false).expect("TODO: ");

                self.modify_position(
                    mint_event.tickLower.unchecked_into(),
                    mint_event.tickUpper.unchecked_into(),
                    mint_event.amount as i128,
                );
                // tracing::debug!(?mint_event, address = ?self.address, sqrt_price = ?self.sqrt_price, liquidity = ?self.liquidity, tick = ?self.tick, "UniswapV3 mint event");
            }
            IUniswapV3PoolEvents::Burn::SIGNATURE_HASH => {
                let burn_event =
                    IUniswapV3PoolEvents::Burn::decode_log(log.as_ref(), false).expect("TODO: ");

                self.modify_position(
                    burn_event.tickLower.unchecked_into(),
                    burn_event.tickUpper.unchecked_into(),
                    -(burn_event.amount as i128),
                );
                // tracing::debug!(?burn_event, address = ?self.address, sqrt_price = ?self.sqrt_price, liquidity = ?self.liquidity, tick = ?self.tick, "UniswapV3 burn event");
            }
            _ => {
                todo!("TODO: Handle this error")
            }
        }
    }

    fn simulate_swap(
        &self,
        base_token: Address,
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        let zero_for_one = base_token == self.token_a;

        // Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one {
            MIN_SQRT_RATIO + U256_1
        } else {
            MAX_SQRT_RATIO - U256_1
        };

        // Initialize a mutable state state struct to hold the dynamic simulated state of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: self.sqrt_price, //Active price on the pool
            amount_calculated: I256::ZERO,    //Amount of token_out that has been calculated
            amount_specified_remaining: I256::from_raw(amount_in), //Amount of token_in that has not been swapped
            tick: self.tick,                                       //Current i24 tick of the pool
            liquidity: self.liquidity, //Current available liquidity in the tick range
        };

        while current_state.amount_specified_remaining != I256::ZERO
            && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96
        {
            // Initialize a new step struct to hold the dynamic state of the pool at each step
            let mut step = StepComputations {
                // Set the sqrt_price_start_x_96 to the current sqrt_price_x_96
                sqrt_price_start_x_96: current_state.sqrt_price_x_96,
                ..Default::default()
            };

            // Get the next tick from the current tick
            (step.tick_next, step.initialized) =
                uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                    &self.tick_bitmap,
                    current_state.tick,
                    self.tick_spacing,
                    zero_for_one,
                )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            // Target spot price
            let swap_target_sqrt_ratio = if zero_for_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            // Compute swap step and update the current state
            (
                current_state.sqrt_price_x_96,
                step.amount_in,
                step.amount_out,
                step.fee_amount,
            ) = uniswap_v3_math::swap_math::compute_swap_step(
                current_state.sqrt_price_x_96,
                swap_target_sqrt_ratio,
                current_state.liquidity,
                current_state.amount_specified_remaining,
                self.fee,
            )?;

            // Decrement the amount remaining to be swapped and amount received from the step
            current_state.amount_specified_remaining = current_state
                .amount_specified_remaining
                .overflowing_sub(I256::from_raw(
                    step.amount_in.overflowing_add(step.fee_amount).0,
                ))
                .0;

            current_state.amount_calculated -= I256::from_raw(step.amount_out);

            // TODO: adjust for fee protocol

            // If the price moved all the way to the next price, recompute the liquidity change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net = if let Some(info) = self.ticks.get(&step.tick_next) {
                        info.liquidity_net
                    } else {
                        0
                    };

                    // we are on a tick boundary, and the next tick is initialized, so we must charge a protocol fee
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Err(AMMError::LiquidityUnderflow);
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                // Increment the current tick
                current_state.tick = if zero_for_one {
                    step.tick_next.wrapping_sub(1)
                } else {
                    step.tick_next
                }
                // If the current_state sqrt price is not equal to the step sqrt price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(
                    current_state.sqrt_price_x_96,
                )?;
            }
        }

        let amount_out = (-current_state.amount_calculated).into_raw();

        tracing::trace!(?amount_out);

        Ok(amount_out)
    }

    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        let zero_for_one = base_token == self.token_a;

        // Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one {
            MIN_SQRT_RATIO + U256_1
        } else {
            MAX_SQRT_RATIO - U256_1
        };

        // Initialize a mutable state state struct to hold the dynamic simulated state of the pool
        let mut current_state = CurrentState {
            // Active price on the pool
            sqrt_price_x_96: self.sqrt_price,
            // Amount of token_out that has been calculated
            amount_calculated: I256::ZERO,
            // Amount of token_in that has not been swapped
            amount_specified_remaining: I256::from_raw(amount_in),
            // Current i24 tick of the pool
            tick: self.tick,
            // Current available liquidity in the tick range
            liquidity: self.liquidity,
        };

        while current_state.amount_specified_remaining != I256::ZERO
            && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96
        {
            // Initialize a new step struct to hold the dynamic state of the pool at each step
            let mut step = StepComputations {
                // Set the sqrt_price_start_x_96 to the current sqrt_price_x_96
                sqrt_price_start_x_96: current_state.sqrt_price_x_96,
                ..Default::default()
            };

            // Get the next tick from the current tick
            (step.tick_next, step.initialized) =
                uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                    &self.tick_bitmap,
                    current_state.tick,
                    self.tick_spacing,
                    zero_for_one,
                )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            // Target spot price
            let swap_target_sqrt_ratio = if zero_for_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            // Compute swap step and update the current state
            (
                current_state.sqrt_price_x_96,
                step.amount_in,
                step.amount_out,
                step.fee_amount,
            ) = uniswap_v3_math::swap_math::compute_swap_step(
                current_state.sqrt_price_x_96,
                swap_target_sqrt_ratio,
                current_state.liquidity,
                current_state.amount_specified_remaining,
                self.fee,
            )?;

            // Decrement the amount remaining to be swapped and amount received from the step
            current_state.amount_specified_remaining = current_state
                .amount_specified_remaining
                .overflowing_sub(I256::from_raw(
                    step.amount_in.overflowing_add(step.fee_amount).0,
                ))
                .0;

            current_state.amount_calculated -= I256::from_raw(step.amount_out);

            // If the price moved all the way to the next price, recompute the liquidity change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net = if let Some(info) = self.ticks.get(&step.tick_next) {
                        info.liquidity_net
                    } else {
                        0
                    };

                    // we are on a tick boundary, and the next tick is initialized, so we must charge a protocol fee
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Err(AMMError::LiquidityUnderflow);
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                // Increment the current tick
                current_state.tick = if zero_for_one {
                    step.tick_next.wrapping_sub(1)
                } else {
                    step.tick_next
                }
                // If the current_state sqrt price is not equal to the step sqrt price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(
                    current_state.sqrt_price_x_96,
                )?;
            }
        }

        // Update the pool state
        self.liquidity = current_state.liquidity;
        self.sqrt_price = current_state.sqrt_price_x_96;
        self.tick = current_state.tick;

        let amount_out = (-current_state.amount_calculated).into_raw();

        tracing::trace!(?amount_out);

        Ok(amount_out)
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.token_a, self.token_b]
    }

    fn calculate_price(&self, base_token: Address, _quote_token: Address) -> Result<f64, AMMError> {
        let tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(self.sqrt_price)?;
        let shift = self.token_a_decimals as i8 - self.token_b_decimals as i8;

        let price = match shift.cmp(&0) {
            Ordering::Less => 1.0001_f64.powi(tick) / 10_f64.powi(-shift as i32),
            Ordering::Greater => 1.0001_f64.powi(tick) * 10_f64.powi(shift as i32),
            Ordering::Equal => 1.0001_f64.powi(tick),
        };

        if base_token == self.token_a {
            Ok(price)
        } else {
            Ok(1.0 / price)
        }
    }
}

impl UniswapV3Pool {
    /// Modifies a positions liquidity in the pool.
    pub fn modify_position(&mut self, tick_lower: i32, tick_upper: i32, liquidity_delta: i128) {
        //We are only using this function when a mint or burn event is emitted,
        //therefore we do not need to checkTicks as that has happened before the event is emitted
        self.update_position(tick_lower, tick_upper, liquidity_delta);

        if liquidity_delta != 0 {
            //if the tick is between the tick lower and tick upper, update the liquidity between the ticks
            if self.tick >= tick_lower && self.tick < tick_upper {
                self.liquidity = if liquidity_delta < 0 {
                    self.liquidity - ((-liquidity_delta) as u128)
                } else {
                    self.liquidity + (liquidity_delta as u128)
                }
            }
        }
    }

    pub fn update_position(&mut self, tick_lower: i32, tick_upper: i32, liquidity_delta: i128) {
        let mut flipped_lower = false;
        let mut flipped_upper = false;

        if liquidity_delta != 0 {
            flipped_lower = self.update_tick(tick_lower, liquidity_delta, false);
            flipped_upper = self.update_tick(tick_upper, liquidity_delta, true);
            if flipped_lower {
                self.flip_tick(tick_lower, self.tick_spacing);
            }
            if flipped_upper {
                self.flip_tick(tick_upper, self.tick_spacing);
            }
        }

        if liquidity_delta < 0 {
            if flipped_lower {
                self.ticks.remove(&tick_lower);
            }

            if flipped_upper {
                self.ticks.remove(&tick_upper);
            }
        }
    }

    pub fn update_tick(&mut self, tick: i32, liquidity_delta: i128, upper: bool) -> bool {
        let info = match self.ticks.get_mut(&tick) {
            Some(info) => info,
            None => {
                self.ticks.insert(tick, Info::default());
                self.ticks
                    .get_mut(&tick)
                    .expect("Tick does not exist in ticks")
            }
        };

        let liquidity_gross_before = info.liquidity_gross;

        let liquidity_gross_after = if liquidity_delta < 0 {
            liquidity_gross_before - ((-liquidity_delta) as u128)
        } else {
            liquidity_gross_before + (liquidity_delta as u128)
        };

        // we do not need to check if liqudity_gross_after > maxLiquidity because we are only calling update tick on a burn or mint log.
        // this should already be validated when a log is
        let flipped = (liquidity_gross_after == 0) != (liquidity_gross_before == 0);

        if liquidity_gross_before == 0 {
            info.initialized = true;
        }

        info.liquidity_gross = liquidity_gross_after;

        info.liquidity_net = if upper {
            info.liquidity_net - liquidity_delta
        } else {
            info.liquidity_net + liquidity_delta
        };

        flipped
    }

    pub fn flip_tick(&mut self, tick: i32, tick_spacing: i32) {
        let (word_pos, bit_pos) = uniswap_v3_math::tick_bitmap::position(tick / tick_spacing);
        let mask = U256::from(1) << bit_pos;

        if let Some(word) = self.tick_bitmap.get_mut(&word_pos) {
            *word ^= mask;
        } else {
            self.tick_bitmap.insert(word_pos, mask);
        }
    }
}

impl Into<AMM> for UniswapV3Pool {
    fn into(self) -> AMM {
        AMM::UniswapV3Pool(self)
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

    async fn get_all_pools<T, N, P>(&self, block_number: u64, provider: Arc<P>) -> Vec<AMM>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let disc_filter = Filter::new()
            .event_signature(FilterSet::from(vec![self.discovery_event()]))
            .address(vec![self.address()]);

        let sync_provider = provider.clone();
        let mut futures = FuturesUnordered::new();

        let sync_step = 100_000;
        let mut latest_block = self.creation_block;
        while latest_block < block_number {
            let mut block_filter = disc_filter.clone();
            let from_block = latest_block;
            let to_block = (from_block + sync_step).min(block_number);

            block_filter = block_filter.from_block(from_block);
            block_filter = block_filter.to_block(to_block);

            let sync_provider = sync_provider.clone();

            futures.push(async move { sync_provider.get_logs(&block_filter).await });

            latest_block = to_block + 1;
        }

        let mut pools = vec![];
        while let Some(res) = futures.next().await {
            let logs = res.expect("TODO: handle error");

            for log in logs {
                pools.push(self.create_pool(log).expect("TODO: handle errors"));
            }
        }

        pools
    }

    async fn sync_all_pools<T, N, P>(pools: &mut [AMM], block_number: u64, provider: Arc<P>)
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        UniswapV3Factory::sync_slot_0(pools, block_number, provider.clone()).await;
        UniswapV3Factory::sync_tick_bitmaps(pools, block_number, provider.clone()).await;
        UniswapV3Factory::sync_tick_data(pools, block_number, provider.clone()).await;
        UniswapV3Factory::sync_token_decimals(pools, provider).await;
    }

    async fn sync_token_decimals<T, N, P>(pools: &mut [AMM], provider: Arc<P>)
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        // Get all token decimals
        let mut tokens = HashSet::new();
        for pool in pools.iter() {
            for token in pool.tokens() {
                tokens.insert(token);
            }
        }
        let token_decimals = get_token_decimals(tokens.into_iter().collect(), provider).await;

        // Set token decimals
        for pool in pools.iter_mut() {
            let AMM::UniswapV3Pool(uniswap_v3_pool) = pool else {
                unreachable!()
            };

            if let Some(decimals) = token_decimals.get(&uniswap_v3_pool.token_a) {
                uniswap_v3_pool.token_a_decimals = *decimals;
            }

            if let Some(decimals) = token_decimals.get(&uniswap_v3_pool.token_b) {
                uniswap_v3_pool.token_b_decimals = *decimals;
            }
        }
    }

    async fn sync_slot_0<T, N, P>(pools: &mut [AMM], block_number: u64, provider: Arc<P>)
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let step = 255;

        let mut futures = FuturesUnordered::new();
        pools.chunks_mut(step).for_each(|group| {
            let provider = provider.clone();
            let pool_addresses = group
                .iter_mut()
                .map(|pool| pool.address())
                .collect::<Vec<_>>();

            futures.push(async move {
                (
                    group,
                    GetUniswapV3PoolSlot0BatchRequest::deploy_builder(provider, pool_addresses)
                        .call_raw()
                        .block(block_number.into())
                        .await
                        .expect("TODO: handle error"),
                )
            });
        });

        let return_type = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
            DynSolType::Int(24),
            DynSolType::Uint(128),
            DynSolType::Uint(256),
        ])));

        while let Some(res) = futures.next().await {
            let (pools, return_data) = res;

            let return_data = return_type
                .abi_decode_sequence(&return_data)
                .expect("TODO: handle error");

            if let Some(tokens_arr) = return_data.as_array() {
                for (slot_0_data, pool) in tokens_arr.iter().zip(pools.iter_mut()) {
                    let AMM::UniswapV3Pool(ref mut uv3_pool) = pool else {
                        unreachable!()
                    };

                    if let Some(slot_0_data) = slot_0_data.as_tuple() {
                        uv3_pool.tick = slot_0_data[0].as_int().expect("TODO:").0.as_i32();
                        uv3_pool.liquidity =
                            slot_0_data[1].as_uint().expect("TODO:").0.to::<u128>();
                        uv3_pool.sqrt_price = slot_0_data[2].as_uint().expect("TODO:").0;
                    }
                }
            }
        }
    }

    async fn sync_tick_bitmaps<T, N, P>(pools: &mut [AMM], block_number: u64, provider: Arc<P>)
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let now = time::Instant::now();
        // TODO: update how we are provisioning the group. We should set a max word pos to fetch and
        // only include as many pools as we can fit in the max word range in a single group

        let mut futures = FuturesUnordered::new();

        let max_range = 15900;
        let mut group_range = 0;
        let mut group = vec![];

        // TODO: collect groups in parallel and then spawn tasks separately
        for pool in pools.iter() {
            let AMM::UniswapV3Pool(uniswap_v3_pool) = pool else {
                unreachable!()
            };

            let mut min_word = tick_to_word(MIN_TICK, uniswap_v3_pool.tick_spacing);
            let max_word = tick_to_word(MAX_TICK, uniswap_v3_pool.tick_spacing);

            // NOTE: found the issue, we are getting max word - min word which is just pos - negative
            let mut word_range = max_word - min_word;

            while word_range > 0 {
                let remaining_range = max_range - group_range;
                let range = word_range.min(remaining_range);

                group.push(TickBitmapInfo {
                    pool: uniswap_v3_pool.address,
                    minWord: min_word as i16,
                    maxWord: (min_word + range) as i16,
                });

                word_range -= range;
                min_word += range - 1;
                group_range += range;

                // If group is full, fire it off and reset
                if group_range >= max_range || word_range <= 0 {
                    let provider = provider.clone();
                    let pool_info = group
                        .iter()
                        .map(|info| (info.pool, info.minWord, info.maxWord))
                        .collect::<Vec<_>>();

                    let calldata = group.drain(..).collect();

                    group_range = 0;

                    futures.push(async move {
                        (
                            pool_info,
                            GetUniswapV3PoolTickBitmapBatchRequest::deploy_builder(
                                provider, calldata,
                            )
                            .call_raw()
                            .block(block_number.into())
                            .await
                            .expect("TODO: handle error"),
                        )
                    });
                }
            }
        }

        let mut pool_set = pools
            .iter_mut()
            .map(|pool| (pool.address(), pool))
            .collect::<HashMap<Address, &mut AMM>>();

        let return_type =
            DynSolType::Array(Box::new(DynSolType::Array(Box::new(DynSolType::Uint(256)))));

        while let Some((pools, return_data)) = futures.next().await {
            let return_data = return_type
                .abi_decode_sequence(&return_data)
                .expect("TODO: handle error");

            if let Some(tokens_arr) = return_data.as_array() {
                for (tick_bitmaps, (pool_address, min_word, max_word)) in
                    tokens_arr.iter().zip(pools.iter())
                {
                    let pool = pool_set.get_mut(pool_address).expect("TODO: handle error");
                    let AMM::UniswapV3Pool(ref mut uv3_pool) = pool else {
                        unreachable!()
                    };

                    // NOTE: we can probably make this more efficient, in amms, we only need applicable tick bitmaps, in this setup we are getting
                    // everything. We can probably filter out words that are not used at some point
                    for (word_pos, bitmap) in
                        (*min_word..=*max_word).zip(tick_bitmaps.as_array().unwrap())
                    {
                        uv3_pool
                            .tick_bitmap
                            .insert(word_pos as i16, bitmap.as_uint().unwrap().0);
                    }
                }
            }
        }
    }

    // TODO: Clean this function up
    async fn sync_tick_data<T, N, P>(pools: &mut [AMM], block_number: u64, provider: Arc<P>)
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let pool_ticks = pools
            .par_iter()
            .filter_map(|pool| {
                if let AMM::UniswapV3Pool(uniswap_v3_pool) = pool {
                    let min_word = tick_to_word(MIN_TICK, uniswap_v3_pool.tick_spacing);
                    let max_word = tick_to_word(MAX_TICK, uniswap_v3_pool.tick_spacing);

                    let tick_bitmaps = (min_word..=max_word)
                        .map(|word_pos| {
                            let bitmap = uniswap_v3_pool.tick_bitmap.get(&(word_pos as i16));
                            (word_pos, bitmap.unwrap_or(&U256::ZERO).clone())
                        })
                        .collect::<Vec<_>>();

                    let mut initialized_ticks = vec![];

                    // NOTE: this needs to be word pos
                    for (word_pos, bitmap) in tick_bitmaps.iter() {
                        if bitmap == &U256::ZERO {
                            continue;
                        }

                        for i in (0..256).filter(|i| {
                            (*bitmap & (U256_1 << U256::from(*i as usize))) != U256::ZERO
                        }) {
                            let tick_index = (word_pos * 256 + i) * uniswap_v3_pool.tick_spacing;

                            if tick_index < MIN_TICK || tick_index > MAX_TICK {
                                panic!("TODO: return error");
                            }

                            initialized_ticks
                                .push(Signed::<24, 1>::from_str(&tick_index.to_string()).unwrap());
                        }
                    }

                    // Only return pools with non-empty initialized ticks
                    if !initialized_ticks.is_empty() {
                        Some((uniswap_v3_pool.address, initialized_ticks))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<(Address, Vec<Signed<24, 1>>)>>();

        let mut futures = FuturesUnordered::new();
        let max_ticks = 200;
        let mut group_ticks = 0;
        let mut group = vec![];

        for (pool_address, mut ticks) in pool_ticks {
            // NOTE: ticks is + 1 too much
            while !ticks.is_empty() {
                let remaining_ticks = max_ticks - group_ticks;
                let selected_ticks = ticks.drain(0..remaining_ticks.min(ticks.len()));
                group_ticks += selected_ticks.len();

                group.push(GetUniswapV3PoolTickDataBatchRequest::TickDataInfo {
                    pool: pool_address,
                    ticks: selected_ticks.collect(),
                });

                if group_ticks >= max_ticks || ticks.is_empty() {
                    let provider = provider.clone();
                    let calldata = group.drain(..).collect::<Vec<TickDataInfo>>();

                    group_ticks = 0;
                    group.clear();

                    futures.push(async move {
                        (
                            calldata.clone(),
                            GetUniswapV3PoolTickDataBatchRequest::deploy_builder(
                                provider, calldata,
                            )
                            .call_raw()
                            .block(block_number.into())
                            .await
                            .expect("TODO: handle error"),
                        )
                    });
                }
            }
        }

        let mut pool_set = pools
            .iter_mut()
            .map(|pool| (pool.address(), pool))
            .collect::<HashMap<Address, &mut AMM>>();

        let return_type = DynSolType::Array(Box::new(DynSolType::Array(Box::new(
            DynSolType::Tuple(vec![
                DynSolType::Uint(128),
                DynSolType::Int(128),
                DynSolType::Bool,
            ]),
        ))));

        while let Some((tick_info, return_data)) = futures.next().await {
            let return_data = return_type
                .abi_decode_sequence(&return_data)
                .expect("TODO: handle error");

            if let Some(tokens_arr) = return_data.as_array() {
                for (tick_bitmaps, tick_info) in tokens_arr.iter().zip(tick_info.iter()) {
                    let pool = pool_set
                        .get_mut(&tick_info.pool)
                        .expect("TODO: handle error");

                    let AMM::UniswapV3Pool(ref mut uv3_pool) = pool else {
                        unreachable!()
                    };

                    if let Some(tick_bitmaps) = tick_bitmaps.as_array() {
                        // TODO: do we need to insert unitilized ticks as well?
                        for (tick, tick_idx) in tick_bitmaps.iter().zip(tick_info.ticks.iter()) {
                            let tick = tick.as_tuple().unwrap();

                            let info = Info {
                                liquidity_gross: tick[0].as_uint().unwrap().0.to::<u128>(),
                                liquidity_net: tick[1].as_int().unwrap().0.try_into().unwrap(),
                                initialized: tick[2].as_bool().unwrap(),
                            };

                            uv3_pool.ticks.insert(tick_idx.as_i32(), info);
                        }
                    }
                }
            }
        }
    }
}

fn tick_to_word(tick: i32, tick_spacing: i32) -> i32 {
    let mut compressed = tick / tick_spacing;
    if tick < 0 && tick % tick_spacing != 0 {
        compressed -= 1;
    }

    compressed >> 8
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
        let pool_created_event: alloy::primitives::Log<IUniswapV3Factory::PoolCreated> =
            IUniswapV3Factory::PoolCreated::decode_log(&log.inner, false).expect("TODO:");

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

impl DiscoverySync for UniswapV3Factory {
    fn discovery_sync<T, N, P>(
        &self,
        to_block: u64,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        async move {
            let mut pools = self.get_all_pools(to_block, provider.clone()).await;
            UniswapV3Factory::sync_all_pools(&mut pools, to_block, provider).await;

            Ok(pools)
        }
    }
}

#[cfg(test)]
mod test {

    use crate::ThrottleLayer;

    use super::*;

    use alloy::{
        primitives::{address, aliases::U24, U160, U256},
        providers::ProviderBuilder,
        rpc::client::ClientBuilder,
        transports::layers::RetryBackoffLayer,
    };

    sol! {
        /// Interface of the Quoter
        #[derive(Debug, PartialEq, Eq)]
        #[sol(rpc)]
        contract IQuoter {
            function quoteExactInputSingle(address tokenIn, address tokenOut,uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external returns (uint256 amountOut);
        }
    }

    async fn usdc_weth_pool<T, N, P>(
        block_number: u64,
        provider: Arc<P>,
    ) -> eyre::Result<UniswapV3Pool>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Clone,
    {
        let pool = AMM::UniswapV3Pool(UniswapV3Pool {
            address: address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
            token_a: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            token_b: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            tick_spacing: 10,
            fee: 500,
            ..Default::default()
        });

        let mut pools = vec![pool];

        UniswapV3Factory::sync_all_pools(&mut pools, block_number, provider).await;

        if let Some(AMM::UniswapV3Pool(pool)) = pools.pop() {
            Ok(pool)
        } else {
            unreachable!()
        }
    }

    async fn weth_link_pool<T, N, P>(
        block_number: u64,
        provider: Arc<P>,
    ) -> eyre::Result<UniswapV3Pool>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Clone,
    {
        let pool = AMM::UniswapV3Pool(UniswapV3Pool {
            address: address!("5d4F3C6fA16908609BAC31Ff148Bd002AA6b8c83"),
            token_a: address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
            token_b: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            tick_spacing: 10,
            fee: 500,
            ..Default::default()
        });

        let mut pools = vec![pool];

        UniswapV3Factory::sync_all_pools(&mut pools, block_number, provider).await;

        if let Some(AMM::UniswapV3Pool(pool)) = pools.pop() {
            Ok(pool)
        } else {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn test_simulate_swap_usdc_weth() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

        let client = ClientBuilder::default()
            .layer(ThrottleLayer::new(250, None)?)
            .layer(RetryBackoffLayer::new(5, 200, 330))
            .http(rpc_endpoint.parse()?);

        let provider = Arc::new(ProviderBuilder::new().on_client(client));

        let current_block = provider.get_block_number().await?;
        let pool = usdc_weth_pool(current_block, provider.clone()).await?;

        let quoter = IQuoter::new(
            address!("b27308f9f90d607463bb33ea1bebb41c27ce5ab6"),
            provider.clone(),
        );

        // Test swap from USDC to WETH

        let amount_in = U256::from(100000000); // 100 USDC
        let amount_out = pool.simulate_swap(pool.token_a, Address::default(), amount_in)?;

        let expected_amount_out = quoter
            .quoteExactInputSingle(
                pool.token_a,
                pool.token_b,
                U24::from(pool.fee),
                amount_in,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out, expected_amount_out.amountOut);

        let amount_in_1 = U256::from(10000000000_u64); // 10_000 USDC
        let amount_out_1 = pool.simulate_swap(pool.token_a, Address::default(), amount_in_1)?;

        let expected_amount_out_1 = quoter
            .quoteExactInputSingle(
                pool.token_a,
                pool.token_b,
                U24::from(pool.fee),
                amount_in_1,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_1, expected_amount_out_1.amountOut);

        let amount_in_2 = U256::from(10000000000000_u128); // 10_000_000 USDC
        let amount_out_2 = pool.simulate_swap(pool.token_a, Address::default(), amount_in_2)?;

        let expected_amount_out_2 = quoter
            .quoteExactInputSingle(
                pool.token_a,
                pool.token_b,
                U24::from(pool.fee),
                amount_in_2,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_2, expected_amount_out_2.amountOut);

        let amount_in_3 = U256::from(100000000000000_u128); // 100_000_000 USDC
        let amount_out_3 = pool.simulate_swap(pool.token_a, Address::default(), amount_in_3)?;

        let expected_amount_out_3 = quoter
            .quoteExactInputSingle(
                pool.token_a,
                pool.token_b,
                U24::from(pool.fee),
                amount_in_3,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_3, expected_amount_out_3.amountOut);

        // Test swap from WETH to USDC

        let amount_in = U256::from(1000000000000000000_u128); // 1 ETH
        let amount_out = pool.simulate_swap(pool.token_b, Address::default(), amount_in)?;
        let expected_amount_out = quoter
            .quoteExactInputSingle(
                pool.token_b,
                pool.token_a,
                U24::from(pool.fee),
                amount_in,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;
        assert_eq!(amount_out, expected_amount_out.amountOut);

        let amount_in_1 = U256::from(10000000000000000000_u128); // 10 ETH
        let amount_out_1 = pool.simulate_swap(pool.token_b, Address::default(), amount_in_1)?;
        let expected_amount_out_1 = quoter
            .quoteExactInputSingle(
                pool.token_b,
                pool.token_a,
                U24::from(pool.fee),
                amount_in_1,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;
        assert_eq!(amount_out_1, expected_amount_out_1.amountOut);

        let amount_in_2 = U256::from(100000000000000000000_u128); // 100 ETH
        let amount_out_2 = pool.simulate_swap(pool.token_b, Address::default(), amount_in_2)?;
        let expected_amount_out_2 = quoter
            .quoteExactInputSingle(
                pool.token_b,
                pool.token_a,
                U24::from(pool.fee),
                amount_in_2,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;
        assert_eq!(amount_out_2, expected_amount_out_2.amountOut);

        let amount_in_3 = U256::from(100000000000000000000_u128); // 100_000 ETH
        let amount_out_3 = pool.simulate_swap(pool.token_b, Address::default(), amount_in_3)?;
        let expected_amount_out_3 = quoter
            .quoteExactInputSingle(
                pool.token_b,
                pool.token_a,
                U24::from(pool.fee),
                amount_in_3,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_3, expected_amount_out_3.amountOut);

        Ok(())
    }

    #[tokio::test]
    async fn test_simulate_swap_link_weth() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

        let client = ClientBuilder::default()
            .layer(ThrottleLayer::new(250, None)?)
            .layer(RetryBackoffLayer::new(5, 200, 330))
            .http(rpc_endpoint.parse()?);

        let provider = Arc::new(ProviderBuilder::new().on_client(client));

        let current_block = provider.get_block_number().await?;
        let pool = weth_link_pool(current_block, provider.clone()).await?;

        let quoter = IQuoter::new(
            address!("b27308f9f90d607463bb33ea1bebb41c27ce5ab6"),
            provider.clone(),
        );

        // Test swap LINK to WETH
        let amount_in = U256::from(1000000000000000000_u128); // 1 LINK
        let amount_out = pool.simulate_swap(pool.token_a, Address::default(), amount_in)?;
        let expected_amount_out = quoter
            .quoteExactInputSingle(
                pool.token_a,
                pool.token_b,
                U24::from(pool.fee),
                amount_in,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out, expected_amount_out.amountOut);

        let amount_in_1 = U256::from(100000000000000000000_u128); // 100 LINK
        let amount_out_1 = pool
            .simulate_swap(pool.token_a, Address::default(), amount_in_1)
            .unwrap();
        let expected_amount_out_1 = quoter
            .quoteExactInputSingle(
                pool.token_a,
                pool.token_b,
                U24::from(pool.fee),
                amount_in_1,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_1, expected_amount_out_1.amountOut);

        let amount_in_2 = U256::from(10000000000000000000000_u128); // 10_000 LINK
        let amount_out_2 = pool
            .simulate_swap(pool.token_a, Address::default(), amount_in_2)
            .unwrap();
        let expected_amount_out_2 = quoter
            .quoteExactInputSingle(
                pool.token_a,
                pool.token_b,
                U24::from(pool.fee),
                amount_in_2,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_2, expected_amount_out_2.amountOut);

        let amount_in_3 = U256::from(10000000000000000000000_u128); // 1_000_000 LINK
        let amount_out_3 = pool
            .simulate_swap(pool.token_a, Address::default(), amount_in_3)
            .unwrap();
        let expected_amount_out_3 = quoter
            .quoteExactInputSingle(
                pool.token_a,
                pool.token_b,
                U24::from(pool.fee),
                amount_in_3,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_3, expected_amount_out_3.amountOut);

        // Test swap WETH to LINK

        let amount_in = U256::from(1000000000000000000_u128); // 1 ETH
        let amount_out = pool.simulate_swap(pool.token_b, Address::default(), amount_in)?;
        let expected_amount_out = quoter
            .quoteExactInputSingle(
                pool.token_b,
                pool.token_a,
                U24::from(pool.fee),
                amount_in,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out, expected_amount_out.amountOut);

        let amount_in_1 = U256::from(10000000000000000000_u128); // 10 ETH
        let amount_out_1 = pool.simulate_swap(pool.token_b, Address::default(), amount_in_1)?;
        let expected_amount_out_1 = quoter
            .quoteExactInputSingle(
                pool.token_b,
                pool.token_a,
                U24::from(pool.fee),
                amount_in_1,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_1, expected_amount_out_1.amountOut);

        let amount_in_2 = U256::from(100000000000000000000_u128); // 100 ETH
        let amount_out_2 = pool.simulate_swap(pool.token_b, Address::default(), amount_in_2)?;
        let expected_amount_out_2 = quoter
            .quoteExactInputSingle(
                pool.token_b,
                pool.token_a,
                U24::from(pool.fee),
                amount_in_2,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;
        assert_eq!(amount_out_2, expected_amount_out_2.amountOut);

        let amount_in_3 = U256::from(100000000000000000000_u128); // 100_000 ETH
        let amount_out_3 = pool.simulate_swap(pool.token_b, Address::default(), amount_in_3)?;
        let expected_amount_out_3 = quoter
            .quoteExactInputSingle(
                pool.token_b,
                pool.token_a,
                U24::from(pool.fee),
                amount_in_3,
                U160::ZERO,
            )
            .block(current_block.into())
            .call()
            .await?;

        assert_eq!(amount_out_3, expected_amount_out_3.amountOut);

        Ok(())
    }

    // TODO: swap sim mut

    #[tokio::test]
    async fn test_calculate_price() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

        let client = ClientBuilder::default()
            .layer(ThrottleLayer::new(250, None)?)
            .layer(RetryBackoffLayer::new(5, 200, 330))
            .http(rpc_endpoint.parse()?);

        let provider = Arc::new(ProviderBuilder::new().on_client(client));

        let block_number = 16515398;
        let pool = usdc_weth_pool(block_number, provider.clone()).await?;

        let float_price_a = pool.calculate_price(pool.token_a, Address::default())?;

        let float_price_b = pool.calculate_price(pool.token_b, Address::default())?;

        assert_eq!(float_price_a, 0.0006081236083117488);
        assert_eq!(float_price_b, 1644.4025299004006);

        Ok(())
    }
}
