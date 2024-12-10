use alloy::primitives::U256;
use rug::Float;

use super::{
    consts::{MPFR_T_PRECISION, U128_0X10000000000000000},
    error::AMMError,
};

pub fn q64_to_float(num: u128) -> Result<f64, AMMError> {
    let float_num = u128_to_float(num)?;
    let divisor = u128_to_float(U128_0X10000000000000000)?;
    Ok((float_num / divisor).to_f64())
}

pub fn u128_to_float(num: u128) -> Result<Float, AMMError> {
    let value_string = num.to_string();
    let parsed_value = Float::parse_radix(value_string, 10)?;
    Ok(Float::with_val(MPFR_T_PRECISION, parsed_value))
}

pub fn u256_to_float(num: U256) -> Result<Float, AMMError> {
    let value_string = num.to_string();
    let parsed_value = Float::parse_radix(value_string, 10)?;
    Ok(Float::with_val(MPFR_T_PRECISION, parsed_value))
}
