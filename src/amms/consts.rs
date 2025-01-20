use alloy::primitives::U256;

// commonly used U256s
pub const U256_10E_10: U256 = U256::from_limbs([10000000000, 0, 0, 0]);
pub const U256_0X100000000: U256 = U256::from_limbs([4294967296, 0, 0, 0]);
pub const U256_0X10000: U256 = U256::from_limbs([65536, 0, 0, 0]);
pub const U256_0X100: U256 = U256::from_limbs([256, 0, 0, 0]);
pub const U256_1000: U256 = U256::from_limbs([1000, 0, 0, 0]);
pub const U256_10000: U256 = U256::from_limbs([10000, 0, 0, 0]);
pub const U256_255: U256 = U256::from_limbs([255, 0, 0, 0]);
pub const U256_192: U256 = U256::from_limbs([192, 0, 0, 0]);
pub const U256_191: U256 = U256::from_limbs([191, 0, 0, 0]);
pub const U256_128: U256 = U256::from_limbs([128, 0, 0, 0]);
pub const U256_64: U256 = U256::from_limbs([64, 0, 0, 0]);
pub const U256_32: U256 = U256::from_limbs([32, 0, 0, 0]);
pub const U256_16: U256 = U256::from_limbs([16, 0, 0, 0]);
pub const U256_10: U256 = U256::from_limbs([10, 0, 0, 0]);
pub const U256_8: U256 = U256::from_limbs([8, 0, 0, 0]);
pub const U256_4: U256 = U256::from_limbs([4, 0, 0, 0]);
pub const U256_2: U256 = U256::from_limbs([2, 0, 0, 0]);
pub const U256_1: U256 = U256::from_limbs([1, 0, 0, 0]);

pub const U256_FEE_ONE: U256 = U256::from_limbs([1_000_000, 0, 0, 0]);
pub const U32_FEE_ONE: u32 = 1_000_000;
pub const F64_FEE_ONE: f64 = 1e6;

// Uniswap V3 specific
pub const POPULATE_TICK_DATA_STEP: u64 = 100000;
pub const Q128: U256 = U256::from_limbs([0, 0, 1, 0]);
pub const Q224: U256 = U256::from_limbs([0, 0, 0, 4294967296]);

// Balancer V2 specific
pub const BONE: U256 = U256::from_limbs([0xDE0B6B3A7640000, 0, 0, 0]);
pub const F64_BONE: f64 = 1e18;
pub const U64_BONE: u64 = 0xDE0B6B3A7640000;

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
pub const U256_0X1FFFFFFFFFFFFF: U256 =
    U256::from_limbs([9007199254740991, 0, 0, 0]); // 2^53 - 1
pub const U256_0X3FFFFFFFFFFFFF: U256 =
    U256::from_limbs([18014398509481983, 0, 0, 0]); // 2^54 - 1

pub const MANTISSA_BITS_F64: i32 = 53;
pub const F64_MAX_SAFE_INTEGER: f64 = 9007199254740991.0; // 2^53 - 1
pub const F64_2P53: f64 = 9007199254740992.0; // 2^53
pub const F64_2P54: f64 = 18014398509481984.0; // 2^54
pub const F64_2P64: f64 = 18446744073709551616.0; // 2^64
pub const F64_2P96: f64 = 79228162514264337593543950336.0; // 2^96
pub const F64_2P128: f64 = 340282366920938463463374607431768211456.0; // 2^128
pub const F64_2P192: f64 = 6277101735386680763835789423207666416102355444464034512896.0; // 2^192
