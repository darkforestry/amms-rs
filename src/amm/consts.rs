use alloy::primitives::U256;

// commonly used U256s
pub const U256_0X100000000: U256 = U256::from_limbs([4294967296, 0, 0, 0]);
pub const U256_0X10000: U256 = U256::from_limbs([65536, 0, 0, 0]);
pub const U256_0X100: U256 = U256::from_limbs([256, 0, 0, 0]);
pub const U256_255: U256 = U256::from_limbs([255, 0, 0, 0]);
pub const U256_192: U256 = U256::from_limbs([192, 0, 0, 0]);
pub const U256_191: U256 = U256::from_limbs([191, 0, 0, 0]);
pub const U256_128: U256 = U256::from_limbs([128, 0, 0, 0]);
pub const U256_64: U256 = U256::from_limbs([64, 0, 0, 0]);
pub const U256_32: U256 = U256::from_limbs([32, 0, 0, 0]);
pub const U256_16: U256 = U256::from_limbs([16, 0, 0, 0]);
pub const U256_8: U256 = U256::from_limbs([8, 0, 0, 0]);
pub const U256_4: U256 = U256::from_limbs([4, 0, 0, 0]);
pub const U256_2: U256 = U256::from_limbs([2, 0, 0, 0]);
pub const U256_1: U256 = U256::from_limbs([1, 0, 0, 0]);

// Uniswap V3 specific
pub const POPULATE_TICK_DATA_STEP: u64 = 100000;
pub const Q128: U256 = U256::from_limbs([0, 0, 1, 0]);
pub const Q224: U256 = U256::from_limbs([0, 0, 0, 4294967296]);

// Others
pub const U128_0X10000000000000000: u128 = 18446744073709551616;
pub const U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: U256 = U256::from_limbs([
    18446744073709551615,
    18446744073709551615,
    18446744073709551615,
    0,
]);
pub const U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: U256 =
    U256::from_limbs([18446744073709551615, 18446744073709551615, 0, 0]);
