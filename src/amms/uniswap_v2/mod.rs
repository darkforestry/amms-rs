use super::{
    amm::{AutomatedMarketMaker, AMM},
    consts::{
        MPFR_T_PRECISION, U128_0X10000000000000000, U256_0X100, U256_0X10000, U256_0X100000000,
        U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF,
        U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF, U256_1, U256_128, U256_16,
        U256_191, U256_192, U256_2, U256_255, U256_32, U256_4, U256_64, U256_8,
    },
    error::AMMError,
    factory::{AutomatedMarketMakerFactory, Factory},
};

use alloy::{
    primitives::{Address, B256, U256},
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
};
use eyre::Result;
use rug::Float;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash};

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

    fn calculate_price(&self, base_token: Address, _quote_token: Address) -> Result<f64, AMMError> {
        let price = self.calculate_price_64_x_64(base_token)?;
        Ok(q64_to_float(price)?)
    }
}

pub fn q64_to_float(num: u128) -> Result<f64, AMMError> {
    let float_num = u128_to_float(num)?;
    let divisor = u128_to_float(U128_0X10000000000000000)?;
    Ok((float_num / divisor).to_f64())
}

pub fn u128_to_float(num: u128) -> Result<Float, AMMError> {
    let value_string = num.to_string();
    let parsed_value =
        Float::parse_radix(value_string, 10).map_err(|_| AMMError::ParseFloatError)?;
    Ok(Float::with_val(MPFR_T_PRECISION, parsed_value))
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

    /// Calculates the price of the base token in terms of the quote token.
    ///
    /// Returned as a Q64 fixed point number.
    pub fn calculate_price_64_x_64(&self, base_token: Address) -> Result<u128, AMMError> {
        let decimal_shift = self.token_a_decimals as i8 - self.token_b_decimals as i8;

        let (r_0, r_1) = if decimal_shift < 0 {
            (
                U256::from(self.reserve_0)
                    * U256::from(10u128.pow(decimal_shift.unsigned_abs() as u32)),
                U256::from(self.reserve_1),
            )
        } else {
            (
                U256::from(self.reserve_0),
                U256::from(self.reserve_1) * U256::from(10u128.pow(decimal_shift as u32)),
            )
        };

        if base_token == self.token_a {
            if r_0.is_zero() {
                Ok(U128_0X10000000000000000)
            } else {
                div_uu(r_1, r_0)
            }
        } else if r_1.is_zero() {
            Ok(U128_0X10000000000000000)
        } else {
            div_uu(r_0, r_1)
        }
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

pub fn div_uu(x: U256, y: U256) -> Result<u128, AMMError> {
    if !y.is_zero() {
        let mut answer;

        if x <= U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            answer = (x << U256_64) / y;
        } else {
            let mut msb = U256_192;
            let mut xc = x >> U256_192;

            if xc >= U256_0X100000000 {
                xc >>= U256_32;
                msb += U256_32;
            }

            if xc >= U256_0X10000 {
                xc >>= U256_16;
                msb += U256_16;
            }

            if xc >= U256_0X100 {
                xc >>= U256_8;
                msb += U256_8;
            }

            if xc >= U256_16 {
                xc >>= U256_4;
                msb += U256_4;
            }

            if xc >= U256_4 {
                xc >>= U256_2;
                msb += U256_2;
            }

            if xc >= U256_2 {
                msb += U256_1;
            }

            answer = (x << (U256_255 - msb)) / (((y - U256_1) >> (msb - U256_191)) + U256_1);
        }

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Ok(0);
        }

        let hi = answer * (y >> U256_128);
        let mut lo = answer * (y & U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);

        let mut xh = x >> U256_192;
        let mut xl = x << U256_64;

        if xl < lo {
            xh -= U256_1;
        }

        xl = xl.overflowing_sub(lo).0;
        lo = hi << U256_128;

        if xl < lo {
            xh -= U256_1;
        }

        xl = xl.overflowing_sub(lo).0;

        if xh != hi >> U256_128 {
            return Err(AMMError::RoundingError);
        }

        answer += xl / y;

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Ok(0_u128);
        }

        Ok(answer.to::<u128>())
    } else {
        Err(AMMError::DivisionByZero)
    }
}
