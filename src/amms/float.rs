use alloy::primitives::U256;

use super::consts::{F64_2P128, F64_2P192, F64_2P64};

/// Converts an alloy U256 to f64 with nearest rounding
pub fn u256_to_f64(num: U256) -> f64 {
    let [l0, l1, l2, l3] = num.into_limbs();
    let (l0f, l1f, l2f, l3f) = (l0 as f64, l1 as f64, l2 as f64, l3 as f64);
    return l0f + l1f * F64_2P64 + l2f * F64_2P128 + l3f * F64_2P192;
}

#[cfg(test)]
mod test {
    use alloy::primitives::U256;

    use crate::amms::{consts::{
        F64_2P54, F64_MAX_SAFE_INTEGER, MANTISSA_BITS_F64, U256_0X10000, U256_0X1FFFFFFFFFFFFF,
        U256_0X3FFFFFFFFFFFFF, U256_1,
    }, float::u256_to_f64};

    #[test]
    fn test_u256_to_f64_simple() {
        assert_eq!(u256_to_f64(U256::ZERO), 0.0);
        assert_eq!(u256_to_f64(U256_1), 1.0);
        assert_eq!(u256_to_f64(U256_0X10000), 65536.0);
    }

    // Make sure that all bits in the input are not lost in the output
    #[test]
    fn test_u256_to_f64_all_bits() {
        for i in 0..256 - MANTISSA_BITS_F64 {
            let actual = u256_to_f64(U256_0X1FFFFFFFFFFFFF << i);
            let expected = F64_MAX_SAFE_INTEGER * (2.0_f64.powi(i));
            assert_eq!(actual, expected, "incorrect bits produced at shift {}", i);
        }
    }

    // Ensures the correct rounding behavior on all positions (should round up)
    #[test]
    fn test_u256_to_f64_rounding() {
        for i in 0..256 - (MANTISSA_BITS_F64 + 1) {
            // Should round 2^54 - 1 up to 2^54 due to 53 bit mantissa
            let actual = u256_to_f64(U256_0X3FFFFFFFFFFFFF << i);
            let expected = F64_2P54 * (2.0_f64.powi(i));
            assert_eq!(actual, expected, "incorrect rounding at shift {}", i);
        }
    }
}
