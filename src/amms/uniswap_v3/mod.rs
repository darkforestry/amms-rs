use super::{
    amm::{AutomatedMarketMaker, AMM},
    error::{AMMError, BatchContractError},
    factory::{AutomatedMarketMakerFactory, DiscoverySync},
    get_token_decimals, Token,
};
use crate::amms::{
    consts::U256_1, uniswap_v3::GetUniswapV3PoolTickBitmapBatchRequest::TickBitmapInfo,
};
use alloy::{
    eips::BlockId,
    network::Network,
    primitives::{Address, Bytes, Signed, B256, I256, U256},
    providers::Provider,
    rpc::types::{Filter, FilterSet, Log},
    sol,
    sol_types::{SolCall, SolEvent, SolValue},
    transports::BoxFuture,
};
use futures::{stream::FuturesUnordered, StreamExt};
use rayon::iter::{IntoParallelRefIterator, ParallelDrainRange, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    cmp::{min, Ordering},
    collections::{HashMap, HashSet},
    future::Future,
    hash::Hash,
    str::FromStr,
};
use thiserror::Error;
use tracing::info;
use uniswap_v3_math::error::UniswapV3MathError;
use uniswap_v3_math::tick_math::{MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK};
use GetUniswapV3PoolTickDataBatchRequest::TickDataInfo;

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


    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract IUniswapV3Pool {
        function swap(address recipient, bool zeroForOne, int256 amountSpecified, uint160 sqrtPriceLimitX96, bytes calldata data) external returns (int256, int256);
        function tickSpacing() external view returns (int24);
        function fee() external view returns (uint24);
        function token0() external view returns (address);
        function token1() external view returns (address);

    }
}

sol! {
    #[sol(rpc)]
    GetUniswapV3PoolSlot0BatchRequest,
    "src/amms/abi/GetUniswapV3PoolSlot0BatchRequest.json",
}

sol! {
    #[sol(rpc)]
    GetUniswapV3PoolTickBitmapBatchRequest,
    "src/amms/abi/GetUniswapV3PoolTickBitmapBatchRequest.json",
}

sol! {
    #[sol(rpc)]
    GetUniswapV3PoolTickDataBatchRequest,
    "src/amms/abi/GetUniswapV3PoolTickDataBatchRequest.json"
}

#[derive(Error, Debug)]
pub enum UniswapV3Error {
    #[error(transparent)]
    UniswapV3MathError(#[from] UniswapV3MathError),
    #[error("Liquidity Underflow")]
    LiquidityUnderflow,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV3Pool {
    pub address: Address,
    pub token_a: Token,
    pub token_b: Token,
    pub liquidity: u128,
    pub sqrt_price: U256,
    pub fee: u32,
    pub tick: i32,
    pub tick_spacing: i32, // TODO: we can make this a u8, tick spacing will never exceed 200
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

    fn sync(&mut self, log: &Log) -> Result<(), AMMError> {
        let event_signature = log.topics()[0];
        match event_signature {
            IUniswapV3PoolEvents::Swap::SIGNATURE_HASH => {
                let swap_event = IUniswapV3PoolEvents::Swap::decode_log(log.as_ref(), false)?;

                self.sqrt_price = swap_event.sqrtPriceX96.to();
                self.liquidity = swap_event.liquidity;
                self.tick = swap_event.tick.unchecked_into();

                info!(
                    target = "amms::uniswap_v3::sync",
                    address = ?self.address,
                    sqrt_price = ?self.sqrt_price,
                    liquidity = ?self.liquidity,
                    tick = ?self.tick,
                    "Swap"
                );
            }
            IUniswapV3PoolEvents::Mint::SIGNATURE_HASH => {
                let mint_event = IUniswapV3PoolEvents::Mint::decode_log(log.as_ref(), false)?;

                self.modify_position(
                    mint_event.tickLower.unchecked_into(),
                    mint_event.tickUpper.unchecked_into(),
                    mint_event.amount as i128,
                )?;

                info!(
                    target = "amms::uniswap_v3::sync",
                    address = ?self.address,
                    sqrt_price = ?self.sqrt_price,
                    liquidity = ?self.liquidity,
                    tick = ?self.tick,
                    "Mint"
                );
            }
            IUniswapV3PoolEvents::Burn::SIGNATURE_HASH => {
                let burn_event = IUniswapV3PoolEvents::Burn::decode_log(log.as_ref(), false)?;

                self.modify_position(
                    burn_event.tickLower.unchecked_into(),
                    burn_event.tickUpper.unchecked_into(),
                    -(burn_event.amount as i128),
                )?;

                info!(
                    target = "amms::uniswap_v3::sync",
                    address = ?self.address,
                    sqrt_price = ?self.sqrt_price,
                    liquidity = ?self.liquidity,
                    tick = ?self.tick,
                    "Burn"
                );
            }
            _ => {
                return Err(AMMError::UnrecognizedEventSignature(event_signature));
            }
        }

        Ok(())
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

        let zero_for_one = base_token == self.token_a.address;

        // Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one {
            MIN_SQRT_RATIO + U256_1
        } else {
            MAX_SQRT_RATIO - U256_1
        };

        // Initialize a mutable state state struct to hold the dynamic simulated state of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: self.sqrt_price, // Active price on the pool
            amount_calculated: I256::ZERO,    // Amount of token_out that has been calculated
            amount_specified_remaining: I256::from_raw(amount_in), // Amount of token_in that has not been swapped
            tick: self.tick,                                       // Current i24 tick of the pool
            liquidity: self.liquidity, // Current available liquidity in the tick range
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
                )
                .map_err(UniswapV3Error::from)?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)
                    .map_err(UniswapV3Error::from)?;

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
            )
            .map_err(UniswapV3Error::from)?;

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
                            return Err(UniswapV3Error::LiquidityUnderflow.into());
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
                )
                .map_err(UniswapV3Error::from)?;
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

        let zero_for_one = base_token == self.token_a.address;

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
                )
                .map_err(UniswapV3Error::from)?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
            // Note: this could be removed as we are clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            // Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)
                    .map_err(UniswapV3Error::from)?;

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
            )
            .map_err(UniswapV3Error::from)?;

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
                            return Err(AMMError::from(UniswapV3Error::LiquidityUnderflow));
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
                )
                .map_err(UniswapV3Error::from)?;
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
        vec![self.token_a.address, self.token_b.address]
    }

    fn calculate_price(&self, base_token: Address, _quote_token: Address) -> Result<f64, AMMError> {
        let tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(self.sqrt_price)
            .map_err(UniswapV3Error::from)?;
        let shift = self.token_a.decimals as i8 - self.token_b.decimals as i8;

        let price = match shift.cmp(&0) {
            Ordering::Less => 1.0001_f64.powi(tick) / 10_f64.powi(-shift as i32),
            Ordering::Greater => 1.0001_f64.powi(tick) * 10_f64.powi(shift as i32),
            Ordering::Equal => 1.0001_f64.powi(tick),
        };

        if base_token == self.token_a.address {
            Ok(price)
        } else {
            Ok(1.0 / price)
        }
    }

    async fn init<N, P>(mut self, block_number: BlockId, provider: P) -> Result<Self, AMMError>
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        let pool = IUniswapV3Pool::new(self.address, provider.clone());

        // Get pool data
        self.tick_spacing = pool.tickSpacing().call().await?._0.as_i32();
        self.fee = pool.fee().call().await?._0.to::<u32>();

        // Get tokens
        self.token_a = Token::new(pool.token0().call().await?._0, provider.clone()).await?;
        self.token_b = Token::new(pool.token1().call().await?._0, provider.clone()).await?;

        let mut pool = vec![self.into()];
        UniswapV3Factory::sync_slot_0(&mut pool, block_number, provider.clone()).await?;
        UniswapV3Factory::sync_token_decimals(&mut pool, provider.clone()).await?;
        UniswapV3Factory::sync_tick_bitmaps(&mut pool, block_number, provider.clone()).await?;
        UniswapV3Factory::sync_tick_data(&mut pool, block_number, provider.clone()).await?;

        let AMM::UniswapV3Pool(pool) = pool[0].to_owned() else {
            unreachable!()
        };

        Ok(pool)
    }
}

impl UniswapV3Pool {
    // Create a new, unsynced UniswapV3 pool
    pub fn new(address: Address) -> Self {
        Self {
            address,
            ..Default::default()
        }
    }

    /// Modifies a positions liquidity in the pool.
    pub fn modify_position(
        &mut self,
        tick_lower: i32,
        tick_upper: i32,
        liquidity_delta: i128,
    ) -> Result<(), AMMError> {
        //We are only using this function when a mint or burn event is emitted,
        //therefore we do not need to checkTicks as that has happened before the event is emitted
        self.update_position(tick_lower, tick_upper, liquidity_delta)?;

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

        Ok(())
    }

    pub fn update_position(
        &mut self,
        tick_lower: i32,
        tick_upper: i32,
        liquidity_delta: i128,
    ) -> Result<(), AMMError> {
        let mut flipped_lower = false;
        let mut flipped_upper = false;

        if liquidity_delta != 0 {
            flipped_lower = self.update_tick(tick_lower, liquidity_delta, false)?;
            flipped_upper = self.update_tick(tick_upper, liquidity_delta, true)?;
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

        Ok(())
    }

    pub fn update_tick(
        &mut self,
        tick: i32,
        liquidity_delta: i128,
        upper: bool,
    ) -> Result<bool, AMMError> {
        let info = self.ticks.entry(tick).or_default();

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

        Ok(flipped)
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

    pub fn swap_calldata(
        &self,
        recipient: Address,
        zero_for_one: bool,
        amount_specified: I256,
        sqrt_price_limit_x_96: U256,
        calldata: Vec<u8>,
    ) -> Result<Bytes, AMMError> {
        Ok(IUniswapV3Pool::swapCall {
            recipient,
            zeroForOne: zero_for_one,
            amountSpecified: amount_specified,
            sqrtPriceLimitX96: sqrt_price_limit_x_96.to(),
            data: calldata.into(),
        }
        .abi_encode()
        .into())
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

    pub async fn get_all_pools<N, P>(
        &self,
        block_number: BlockId,
        provider: P,
    ) -> Result<Vec<AMM>, AMMError>
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        let disc_filter = Filter::new()
            .event_signature(FilterSet::from(vec![self.pool_creation_event()]))
            .address(vec![self.address()]);

        let sync_provider = provider.clone();
        let mut futures = FuturesUnordered::new();

        let sync_step = 100_000;
        let mut latest_block = self.creation_block;
        while latest_block < block_number.as_u64().unwrap_or_default() {
            let mut block_filter = disc_filter.clone();
            let from_block = latest_block;
            let to_block = (from_block + sync_step).min(block_number.as_u64().unwrap_or_default());

            block_filter = block_filter.from_block(from_block);
            block_filter = block_filter.to_block(to_block);

            let sync_provider = sync_provider.clone();

            futures.push(async move { sync_provider.get_logs(&block_filter).await });

            latest_block = to_block + 1;
        }

        let mut pools = vec![];
        while let Some(res) = futures.next().await {
            let logs = res?;

            for log in logs {
                pools.push(self.create_pool(log)?);
            }
        }

        Ok(pools)
    }

    pub async fn sync_all_pools<N, P>(
        mut pools: Vec<AMM>,
        block_number: BlockId,
        provider: P,
    ) -> Result<Vec<AMM>, AMMError>
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        UniswapV3Factory::sync_slot_0(&mut pools, block_number, provider.clone()).await?;
        UniswapV3Factory::sync_token_decimals(&mut pools, provider.clone()).await?;

        pools = pools
            .par_drain(..)
            .filter(|pool| match pool {
                AMM::UniswapV3Pool(uv3_pool) => {
                    uv3_pool.liquidity > 0
                        && uv3_pool.token_a.decimals > 0
                        && uv3_pool.token_b.decimals > 0
                }
                _ => true,
            })
            .collect();

        UniswapV3Factory::sync_tick_bitmaps(&mut pools, block_number, provider.clone()).await?;
        UniswapV3Factory::sync_tick_data(&mut pools, block_number, provider.clone()).await?;

        Ok(pools)
    }

    async fn sync_token_decimals<N, P>(
        pools: &mut [AMM],
        provider: P,
    ) -> Result<(), BatchContractError>
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        // Get all token decimals
        let mut tokens = HashSet::new();
        for pool in pools.iter() {
            for token in pool.tokens() {
                tokens.insert(token);
            }
        }
        let token_decimals = get_token_decimals(tokens.into_iter().collect(), provider).await?;

        // Set token decimals
        for pool in pools.iter_mut() {
            let AMM::UniswapV3Pool(uniswap_v3_pool) = pool else {
                unreachable!()
            };

            if let Some(decimals) = token_decimals.get(&uniswap_v3_pool.token_a.address) {
                uniswap_v3_pool.token_a.decimals = *decimals;
            }

            if let Some(decimals) = token_decimals.get(&uniswap_v3_pool.token_b.address) {
                uniswap_v3_pool.token_b.decimals = *decimals;
            }
        }

        Ok(())
    }

    async fn sync_slot_0<N, P>(
        pools: &mut [AMM],
        block_number: BlockId,
        provider: P,
    ) -> Result<(), AMMError>
    where
        N: Network,
        P: Provider<N> + Clone,
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
                Ok::<(&mut [AMM], Bytes), AMMError>((
                    group,
                    GetUniswapV3PoolSlot0BatchRequest::deploy_builder(provider, pool_addresses)
                        .call_raw()
                        .block(block_number)
                        .await?,
                ))
            });
        });

        while let Some(res) = futures.next().await {
            let (pools, return_data) = res?;
            let return_data =
                <Vec<(i32, u128, U256)> as SolValue>::abi_decode(&return_data, false)?;

            for (slot_0_data, pool) in return_data.iter().zip(pools.iter_mut()) {
                let AMM::UniswapV3Pool(ref mut uv3_pool) = pool else {
                    unreachable!()
                };

                uv3_pool.tick = slot_0_data.0;
                uv3_pool.liquidity = slot_0_data.1;
                uv3_pool.sqrt_price = slot_0_data.2;
            }
        }

        Ok(())
    }

    async fn sync_tick_bitmaps<N, P>(
        pools: &mut [AMM],
        block_number: BlockId,
        provider: P,
    ) -> Result<(), AMMError>
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        let mut futures: FuturesUnordered<BoxFuture<'_, _>> = FuturesUnordered::new();

        let max_group_size = 90;
        let max_group_words = 6900;
        let mut curr_words = 0;
        let mut curr_group = vec![];

        // Batched, limited to max_group_size range queries per group and max_group_words over all ranges
        for pool in pools.iter() {
            let AMM::UniswapV3Pool(uniswap_v3_pool) = pool else {
                unreachable!()
            };

            let mut min_word = tick_to_word(MIN_TICK, uniswap_v3_pool.tick_spacing);
            let max_word = tick_to_word(MAX_TICK, uniswap_v3_pool.tick_spacing);

            while min_word <= max_word {
                let remaining_group_words = max_group_words - curr_words;
                let remaining_pool_words = max_word - min_word + 1;
                let additional_words = min(remaining_group_words, remaining_pool_words);

                // Query [min_word, max_word] (inclusive)
                curr_group.push(TickBitmapInfo {
                    pool: uniswap_v3_pool.address,
                    minWord: min_word as i16,
                    maxWord: (min_word + additional_words - 1) as i16,
                });

                curr_words += additional_words;
                min_word += additional_words;

                // If group is full, fire it off and reset
                if curr_words >= max_group_words || curr_group.len() + 1 >= max_group_size {
                    let provider = provider.clone();
                    let pool_info = curr_group.iter().map(|info| info.pool).collect::<Vec<_>>();

                    let calldata = std::mem::take(&mut curr_group);

                    curr_words = 0;

                    futures.push(Box::pin(async move {
                        Ok::<(Vec<Address>, Bytes), AMMError>((
                            pool_info,
                            GetUniswapV3PoolTickBitmapBatchRequest::deploy_builder(
                                provider, calldata,
                            )
                            .call_raw()
                            .block(block_number)
                            .await?,
                        ))
                    }));
                }
            }
        }

        // Flush remaining queries in group if not empty
        if !curr_group.is_empty() {
            let provider = provider.clone();
            let pool_info = curr_group.iter().map(|info| info.pool).collect::<Vec<_>>();

            let calldata = std::mem::take(&mut curr_group);

            futures.push(Box::pin(async move {
                Ok::<(Vec<Address>, Bytes), AMMError>((
                    pool_info,
                    GetUniswapV3PoolTickBitmapBatchRequest::deploy_builder(provider, calldata)
                        .call_raw()
                        .block(block_number)
                        .await?,
                ))
            }));
        }

        let mut pool_set = pools
            .iter_mut()
            .map(|pool| (pool.address(), pool))
            .collect::<HashMap<Address, &mut AMM>>();

        while let Some(res) = futures.next().await {
            let (pools, return_data) = res?;
            let return_data = <Vec<Vec<U256>> as SolValue>::abi_decode(&return_data, false)?;

            for (tick_bitmaps, pool_address) in return_data.iter().zip(pools.iter()) {
                let pool = pool_set.get_mut(pool_address).unwrap();

                let AMM::UniswapV3Pool(ref mut uv3_pool) = pool else {
                    unreachable!()
                };

                for chunk in tick_bitmaps.chunks_exact(2) {
                    let word_pos = I256::from_raw(chunk[0]).as_i16();
                    let tick_bitmap = chunk[1];

                    uv3_pool.tick_bitmap.insert(word_pos, tick_bitmap);
                }
            }
        }
        Ok(())
    }

    // TODO: Clean this function up
    async fn sync_tick_data<N, P>(
        pools: &mut [AMM],
        block_number: BlockId,
        provider: P,
    ) -> Result<(), AMMError>
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        let pool_ticks = pools
            .par_iter()
            .filter_map(|pool| {
                if let AMM::UniswapV3Pool(uniswap_v3_pool) = pool {
                    let min_word = tick_to_word(MIN_TICK, uniswap_v3_pool.tick_spacing);
                    let max_word = tick_to_word(MAX_TICK, uniswap_v3_pool.tick_spacing);

                    let initialized_ticks: Vec<Signed<24, 1>> = (min_word..=max_word)
                        // Filter out empty bitmaps
                        .filter_map(|word_pos| {
                            uniswap_v3_pool
                                .tick_bitmap
                                .get(&(word_pos as i16))
                                .filter(|&bitmap| *bitmap != U256::ZERO)
                                .map(|&bitmap| (word_pos, bitmap))
                        })
                        // Get tick index for non zero bitmaps
                        .flat_map(|(word_pos, bitmap)| {
                            (0..256)
                                .filter(move |i| {
                                    (bitmap & (U256::from(1) << U256::from(*i))) != U256::ZERO
                                })
                                .map(move |i| {
                                    let tick_index =
                                        (word_pos * 256 + i) * uniswap_v3_pool.tick_spacing;

                                    // TODO: update to use from be bytes or similar
                                    Signed::<24, 1>::from_str(&tick_index.to_string()).unwrap()
                                })
                        })
                        .collect();

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

        let mut futures: FuturesUnordered<BoxFuture<'_, _>> = FuturesUnordered::new();
        let max_ticks = 60;
        let mut group_ticks = 0;
        let mut group = vec![];

        for (pool_address, mut ticks) in pool_ticks {
            while !ticks.is_empty() {
                let remaining_ticks = max_ticks - group_ticks;
                let selected_ticks = ticks.drain(0..remaining_ticks.min(ticks.len()));
                group_ticks += selected_ticks.len();

                group.push(GetUniswapV3PoolTickDataBatchRequest::TickDataInfo {
                    pool: pool_address,
                    ticks: selected_ticks.collect(),
                });

                if group_ticks >= max_ticks {
                    let provider = provider.clone();
                    let calldata = std::mem::take(&mut group);

                    group_ticks = 0;
                    group.clear();

                    futures.push(Box::pin(async move {
                        Ok::<(Vec<TickDataInfo>, Bytes), AMMError>((
                            calldata.clone(),
                            GetUniswapV3PoolTickDataBatchRequest::deploy_builder(
                                provider, calldata,
                            )
                            .call_raw()
                            .block(block_number)
                            .await?,
                        ))
                    }));
                }
            }
        }

        if !group.is_empty() {
            let provider = provider.clone();
            let calldata = std::mem::take(&mut group);

            futures.push(Box::pin(async move {
                Ok::<(Vec<TickDataInfo>, Bytes), AMMError>((
                    calldata.clone(),
                    GetUniswapV3PoolTickDataBatchRequest::deploy_builder(provider, calldata)
                        .call_raw()
                        .block(block_number)
                        .await?,
                ))
            }));
        }

        let mut pool_set = pools
            .iter_mut()
            .map(|pool| (pool.address(), pool))
            .collect::<HashMap<Address, &mut AMM>>();

        while let Some(res) = futures.next().await {
            let (tick_info, return_data) = res?;
            let return_data =
                <Vec<Vec<(bool, u128, i128)>> as SolValue>::abi_decode(&return_data, false)?;

            for (tick_bitmaps, tick_info) in return_data.iter().zip(tick_info.iter()) {
                let pool = pool_set.get_mut(&tick_info.pool).unwrap();

                let AMM::UniswapV3Pool(ref mut uv3_pool) = pool else {
                    unreachable!()
                };

                for (tick, tick_idx) in tick_bitmaps.iter().zip(tick_info.ticks.iter()) {
                    let info = Info {
                        liquidity_gross: tick.1,
                        liquidity_net: tick.2,
                        initialized: tick.0,
                    };

                    uv3_pool.ticks.insert(tick_idx.as_i32(), info);
                }
            }
        }
        Ok(())
    }
}

fn tick_to_word(tick: i32, tick_spacing: i32) -> i32 {
    let mut compressed = tick / tick_spacing;
    if tick < 0 && tick % tick_spacing != 0 {
        compressed -= 1;
    }

    compressed >> 8
}

impl AutomatedMarketMakerFactory for UniswapV3Factory {
    type PoolVariant = UniswapV3Pool;

    fn address(&self) -> Address {
        self.address
    }

    fn pool_creation_event(&self) -> B256 {
        IUniswapV3Factory::PoolCreated::SIGNATURE_HASH
    }

    fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
        let pool_created_event: alloy::primitives::Log<IUniswapV3Factory::PoolCreated> =
            IUniswapV3Factory::PoolCreated::decode_log(&log.inner, false)?;

        Ok(AMM::UniswapV3Pool(UniswapV3Pool {
            address: pool_created_event.pool,
            token_a: pool_created_event.token0.into(),
            token_b: pool_created_event.token1.into(),
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
    fn discover<N, P>(
        &self,
        to_block: BlockId,
        provider: P,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        info!(
            target = "amms::uniswap_v3::discover",
            address = ?self.address,
            "Discovering all pools"
        );

        self.get_all_pools(to_block, provider.clone())
    }

    fn sync<N, P>(
        &self,
        amms: Vec<AMM>,
        to_block: BlockId,
        provider: P,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        info!(
            target = "amms::uniswap_v3::sync",
            address = ?self.address,
            "Syncing all pools"
        );

        UniswapV3Factory::sync_all_pools(amms, to_block, provider)
    }
}

#[cfg(test)]
mod test {

    use super::*;

    use alloy::{
        primitives::{address, aliases::U24, U160, U256},
        providers::ProviderBuilder,
        rpc::client::ClientBuilder,
        transports::layers::{RetryBackoffLayer, ThrottleLayer},
    };

    sol! {
        /// Interface of the Quoter
        #[derive(Debug, PartialEq, Eq)]
        #[sol(rpc)]
        contract IQuoter {
            function quoteExactInputSingle(address tokenIn, address tokenOut,uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external returns (uint256 amountOut);
        }
    }

    #[tokio::test]
    async fn test_simulate_swap_usdc_weth() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

        let client = ClientBuilder::default()
            .layer(ThrottleLayer::new(250))
            .layer(RetryBackoffLayer::new(5, 200, 330))
            .http(rpc_endpoint.parse()?);

        let provider = ProviderBuilder::new().on_client(client);

        let pool = UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"))
            .init(BlockId::latest(), provider.clone())
            .await?;

        let quoter = IQuoter::new(
            address!("b27308f9f90d607463bb33ea1bebb41c27ce5ab6"),
            provider.clone(),
        );

        // Test swap from USDC to WETH
        let amount_in = U256::from(100000000); // 100 USDC
        let amount_out = pool.simulate_swap(pool.token_a.address, Address::default(), amount_in)?;

        dbg!(pool.token_a.address);
        dbg!(pool.token_b.address);
        dbg!(amount_in);
        dbg!(amount_out);
        dbg!(pool.fee);

        let expected_amount_out = quoter
            .quoteExactInputSingle(
                pool.token_a.address,
                pool.token_b.address,
                U24::from(pool.fee),
                amount_in,
                U160::ZERO,
            )
            .block(BlockId::latest())
            .call()
            .await?;

        assert_eq!(amount_out, expected_amount_out.amountOut);

        let amount_in_1 = U256::from(10000000000_u64); // 10_000 USDC
        let amount_out_1 =
            pool.simulate_swap(pool.token_a.address, Address::default(), amount_in_1)?;

        let expected_amount_out_1 = quoter
            .quoteExactInputSingle(
                pool.token_a.address,
                pool.token_b.address,
                U24::from(pool.fee),
                amount_in_1,
                U160::ZERO,
            )
            .block(BlockId::latest())
            .call()
            .await?;

        assert_eq!(amount_out_1, expected_amount_out_1.amountOut);

        let amount_in_2 = U256::from(10000000000000_u128); // 10_000_000 USDC
        let amount_out_2 =
            pool.simulate_swap(pool.token_a.address, Address::default(), amount_in_2)?;

        let expected_amount_out_2 = quoter
            .quoteExactInputSingle(
                pool.token_a.address,
                pool.token_b.address,
                U24::from(pool.fee),
                amount_in_2,
                U160::ZERO,
            )
            .block(BlockId::latest())
            .call()
            .await?;

        assert_eq!(amount_out_2, expected_amount_out_2.amountOut);

        let amount_in_3 = U256::from(100000000000000_u128); // 100_000_000 USDC
        let amount_out_3 =
            pool.simulate_swap(pool.token_a.address, Address::default(), amount_in_3)?;

        let expected_amount_out_3 = quoter
            .quoteExactInputSingle(
                pool.token_a.address,
                pool.token_b.address,
                U24::from(pool.fee),
                amount_in_3,
                U160::ZERO,
            )
            .block(BlockId::latest())
            .call()
            .await?;

        assert_eq!(amount_out_3, expected_amount_out_3.amountOut);

        // Test swap from WETH to USDC

        let amount_in = U256::from(1000000000000000000_u128); // 1 ETH
        let amount_out = pool.simulate_swap(pool.token_b.address, Address::default(), amount_in)?;
        let expected_amount_out = quoter
            .quoteExactInputSingle(
                pool.token_b.address,
                pool.token_a.address,
                U24::from(pool.fee),
                amount_in,
                U160::ZERO,
            )
            .block(BlockId::latest())
            .call()
            .await?;
        assert_eq!(amount_out, expected_amount_out.amountOut);

        let amount_in_1 = U256::from(10000000000000000000_u128); // 10 ETH
        let amount_out_1 =
            pool.simulate_swap(pool.token_b.address, Address::default(), amount_in_1)?;
        let expected_amount_out_1 = quoter
            .quoteExactInputSingle(
                pool.token_b.address,
                pool.token_a.address,
                U24::from(pool.fee),
                amount_in_1,
                U160::ZERO,
            )
            .block(BlockId::latest())
            .call()
            .await?;
        assert_eq!(amount_out_1, expected_amount_out_1.amountOut);

        let amount_in_2 = U256::from(100000000000000000000_u128); // 100 ETH
        let amount_out_2 =
            pool.simulate_swap(pool.token_b.address, Address::default(), amount_in_2)?;
        let expected_amount_out_2 = quoter
            .quoteExactInputSingle(
                pool.token_b.address,
                pool.token_a.address,
                U24::from(pool.fee),
                amount_in_2,
                U160::ZERO,
            )
            .block(BlockId::latest())
            .call()
            .await?;
        assert_eq!(amount_out_2, expected_amount_out_2.amountOut);

        let amount_in_3 = U256::from(100000000000000000000_u128); // 100_000 ETH
        let amount_out_3 =
            pool.simulate_swap(pool.token_b.address, Address::default(), amount_in_3)?;
        let expected_amount_out_3 = quoter
            .quoteExactInputSingle(
                pool.token_b.address,
                pool.token_a.address,
                U24::from(pool.fee),
                amount_in_3,
                U160::ZERO,
            )
            .block(BlockId::latest())
            .call()
            .await?;

        assert_eq!(amount_out_3, expected_amount_out_3.amountOut);

        Ok(())
    }

    #[tokio::test]
    async fn test_simulate_swap_link_weth() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

        let client = ClientBuilder::default()
            .layer(ThrottleLayer::new(250))
            .layer(RetryBackoffLayer::new(5, 200, 330))
            .http(rpc_endpoint.parse()?);

        let provider = ProviderBuilder::new().on_client(client);

        let current_block = BlockId::from(provider.get_block_number().await?);

        let pool = UniswapV3Pool::new(address!("5d4F3C6fA16908609BAC31Ff148Bd002AA6b8c83"))
            .init(current_block, provider.clone())
            .await?;

        let quoter = IQuoter::new(
            address!("b27308f9f90d607463bb33ea1bebb41c27ce5ab6"),
            provider.clone(),
        );

        // Test swap LINK to WETH
        let amount_in = U256::from(1000000000000000000_u128); // 1 LINK
        let amount_out = pool.simulate_swap(pool.token_a.address, Address::default(), amount_in)?;
        let expected_amount_out = quoter
            .quoteExactInputSingle(
                pool.token_a.address,
                pool.token_b.address,
                U24::from(pool.fee),
                amount_in,
                U160::ZERO,
            )
            .block(current_block)
            .call()
            .await?;

        assert_eq!(amount_out, expected_amount_out.amountOut);

        let amount_in_1 = U256::from(100000000000000000000_u128); // 100 LINK
        let amount_out_1 = pool
            .simulate_swap(pool.token_a.address, Address::default(), amount_in_1)
            .unwrap();
        let expected_amount_out_1 = quoter
            .quoteExactInputSingle(
                pool.token_a.address,
                pool.token_b.address,
                U24::from(pool.fee),
                amount_in_1,
                U160::ZERO,
            )
            .block(current_block)
            .call()
            .await?;

        assert_eq!(amount_out_1, expected_amount_out_1.amountOut);

        let amount_in_2 = U256::from(10000000000000000000000_u128); // 10_000 LINK
        let amount_out_2 = pool
            .simulate_swap(pool.token_a.address, Address::default(), amount_in_2)
            .unwrap();
        let expected_amount_out_2 = quoter
            .quoteExactInputSingle(
                pool.token_a.address,
                pool.token_b.address,
                U24::from(pool.fee),
                amount_in_2,
                U160::ZERO,
            )
            .block(current_block)
            .call()
            .await?;

        assert_eq!(amount_out_2, expected_amount_out_2.amountOut);

        let amount_in_3 = U256::from(10000000000000000000000_u128); // 1_000_000 LINK
        let amount_out_3 = pool
            .simulate_swap(pool.token_a.address, Address::default(), amount_in_3)
            .unwrap();
        let expected_amount_out_3 = quoter
            .quoteExactInputSingle(
                pool.token_a.address,
                pool.token_b.address,
                U24::from(pool.fee),
                amount_in_3,
                U160::ZERO,
            )
            .block(current_block)
            .call()
            .await?;

        assert_eq!(amount_out_3, expected_amount_out_3.amountOut);

        // Test swap WETH to LINK

        let amount_in = U256::from(1000000000000000000_u128); // 1 ETH
        let amount_out = pool.simulate_swap(pool.token_b.address, Address::default(), amount_in)?;
        let expected_amount_out = quoter
            .quoteExactInputSingle(
                pool.token_b.address,
                pool.token_a.address,
                U24::from(pool.fee),
                amount_in,
                U160::ZERO,
            )
            .block(current_block)
            .call()
            .await?;

        assert_eq!(amount_out, expected_amount_out.amountOut);

        let amount_in_1 = U256::from(10000000000000000000_u128); // 10 ETH
        let amount_out_1 =
            pool.simulate_swap(pool.token_b.address, Address::default(), amount_in_1)?;
        let expected_amount_out_1 = quoter
            .quoteExactInputSingle(
                pool.token_b.address,
                pool.token_a.address,
                U24::from(pool.fee),
                amount_in_1,
                U160::ZERO,
            )
            .block(current_block)
            .call()
            .await?;

        assert_eq!(amount_out_1, expected_amount_out_1.amountOut);

        let amount_in_2 = U256::from(100000000000000000000_u128); // 100 ETH
        let amount_out_2 =
            pool.simulate_swap(pool.token_b.address, Address::default(), amount_in_2)?;
        let expected_amount_out_2 = quoter
            .quoteExactInputSingle(
                pool.token_b.address,
                pool.token_a.address,
                U24::from(pool.fee),
                amount_in_2,
                U160::ZERO,
            )
            .block(current_block)
            .call()
            .await?;
        assert_eq!(amount_out_2, expected_amount_out_2.amountOut);

        let amount_in_3 = U256::from(100000000000000000000_u128); // 100_000 ETH
        let amount_out_3 =
            pool.simulate_swap(pool.token_b.address, Address::default(), amount_in_3)?;
        let expected_amount_out_3 = quoter
            .quoteExactInputSingle(
                pool.token_b.address,
                pool.token_a.address,
                U24::from(pool.fee),
                amount_in_3,
                U160::ZERO,
            )
            .block(current_block)
            .call()
            .await?;

        assert_eq!(amount_out_3, expected_amount_out_3.amountOut);

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_price() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

        let client = ClientBuilder::default()
            .layer(ThrottleLayer::new(250))
            .layer(RetryBackoffLayer::new(5, 200, 330))
            .http(rpc_endpoint.parse()?);

        let provider = ProviderBuilder::new().on_client(client);

        let block_number = BlockId::from(22000114);
        let pool = UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"))
            .init(block_number, provider.clone())
            .await?;

        let float_price_a = pool.calculate_price(pool.token_a.address, Address::default())?;
        let float_price_b = pool.calculate_price(pool.token_b.address, Address::default())?;
        assert_eq!(float_price_a, 0.00046777681145863687);
        assert_eq!(float_price_b, 2137.7716370372605);

        Ok(())
    }
}
