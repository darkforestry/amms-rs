use alloy::primitives::U256;
use rug::Float;

use crate::amm::consts::{BONE, DECIMAL_RADIX, MPFR_T_PRECISION, U256_2};

pub fn badd(a: U256, b: U256) -> U256 {
    let c = a + b;
    assert!(c >= a, "ERR_ADD_OVERFLOW");
    c
}
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

/**********************************************************************************************
// calcSpotPrice                                                                             //
// sP = spotPrice                                                                            //
// bI = tokenBalanceIn                ( bI / wI )         1                                  //
// bO = tokenBalanceOut         sP =  -----------  *  ----------                             //
// wI = tokenWeightIn                 ( bO / wO )     ( 1 - sF )                             //
// wO = tokenWeightOut                                                                       //
// sF = swapFee                                                                              //
 **********************************************************************************************/
pub fn calculate_price(b_i: U256, w_i: U256, b_o: U256, w_o: U256, s_f: U256) -> U256 {
    let numer = bdiv(b_i, w_i);
    let denom = bdiv(b_o, w_o);
    let ratio = bdiv(numer, denom);
    let scale = bdiv(BONE, bsub(BONE, s_f));
    bmul(ratio, scale)
}

/**********************************************************************************************
// calcOutGivenIn                                                                            //
// aO = tokenAmountOut                                                                       //
// bO = tokenBalanceOut                                                                      //
// bI = tokenBalanceIn              /      /            bI             \    (wI / wO) \      //
// aI = tokenAmountIn    aO = bO * |  1 - | --------------------------  | ^            |     //
// wI = tokenWeightIn               \      \ ( bI + ( aI * ( 1 - sF )) /              /      //
// wO = tokenWeightOut                                                                       //
// sF = swapFee                                                                              //
 **********************************************************************************************/
//  function calcOutGivenIn(
//     uint tokenBalanceIn,
//     uint tokenWeightIn,
//     uint tokenBalanceOut,
//     uint tokenWeightOut,
//     uint tokenAmountIn,
//     uint swapFee
// )
//     public pure
//     returns (uint tokenAmountOut)
// {
//     uint weightRatio = bdiv(tokenWeightIn, tokenWeightOut);
//     uint adjustedIn = bsub(BONE, swapFee);
//     adjustedIn = bmul(tokenAmountIn, adjustedIn);
//     uint y = bdiv(tokenBalanceIn, badd(tokenBalanceIn, adjustedIn));
//     uint foo = bpow(y, weightRatio);
//     uint bar = bsub(BONE, foo);
//     tokenAmountOut = bmul(tokenBalanceOut, bar);
//     return tokenAmountOut;
// }
pub fn calculate_out_given_in(
    token_balance_in: U256,
    token_weight_in: U256,
    token_balance_out: U256,
    token_weight_out: U256,
    token_amount_in: U256,
    swap_fee: U256,
) -> U256 {
    let weight_ratio = bdiv(token_weight_in, token_weight_out);
    let adjusted_in = bsub(BONE, swap_fee);
    let adjusted_in = bmul(token_amount_in, adjusted_in);
    let y = bdiv(token_balance_in, badd(token_balance_in, adjusted_in));
    let foo = y.pow(weight_ratio);
    let bar = bsub(BONE, foo);
    bmul(token_balance_out, bar)
}

/// Converts a `U256` into a `Float` with a high precision.
pub fn u256_to_float(value: U256) -> Float {
    // convert U256 to a string - represented as a decimal string number
    let value_string = value.to_string();
    let parsed_value = Float::parse_radix(value_string, DECIMAL_RADIX)
        .expect("U256 is always converted into a decimal string number.");
    Float::with_val(MPFR_T_PRECISION, parsed_value)
}
