use alloy::primitives::U256;

use crate::amm::consts::{BONE, U256_2};

// Reference:
// https://github.com/balancer/balancer-core/blob/f4ed5d65362a8d6cec21662fb6eae233b0babc1f/contracts/BNum.sol#L75
pub fn bdiv(a: U256, b: U256) -> U256 {
    assert!(b != U256::ZERO, "ERR_DIV_ZERO");
    let c0 = a * BONE;
    assert!(a == U256::ZERO || c0 / a == BONE, "ERR_DIV_INTERNAL");
    let c1 = c0 + (b / U256_2);
    assert!(c1 >= c0, "ERR_DIV_INTERNAL");
    c1 / b
}

// Reference:
// https://github.com/balancer/balancer-core/blob/f4ed5d65362a8d6cec21662fb6eae233b0babc1f/contracts/BNum.sol#L43
pub fn bsub(a: U256, b: U256) -> U256 {
    let (c, flag) = bsub_sign(a, b);
    assert!(!flag, "ERR_SUB_UNDERFLOW");
    c
}

// Reference:
// https://github.com/balancer/balancer-core/blob/f4ed5d65362a8d6cec21662fb6eae233b0babc1f/contracts/BNum.sol#L52
pub fn bsub_sign(a: U256, b: U256) -> (U256, bool) {
    if a >= b {
        (a - b, false)
    } else {
        (b - a, true)
    }
}

// Reference:
// https://github.com/balancer/balancer-core/blob/f4ed5d65362a8d6cec21662fb6eae233b0babc1f/contracts/BNum.sol#L63C4-L73C6
pub fn bmul(a: U256, b: U256) -> U256 {
    let c0 = a * b;
    assert!(a == U256::ZERO || c0 / a == b, "ERR_MUL_OVERFLOW");
    let c1 = c0 + (BONE / U256_2);
    assert!(c1 >= c0, "ERR_MUL_OVERFLOW");
    c1 / BONE
}
