use crate::pool::ArithmeticError;
use ethers::types::U256;
use num_bigfloat::BigFloat;
pub const U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: U256 = U256([
    18446744073709551615,
    18446744073709551615,
    18446744073709551615,
    0,
]);

pub const U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: U256 =
    U256([18446744073709551615, 18446744073709551615, 0, 0]);
pub const U128_0X10000000000000000: u128 = 18446744073709551616;

pub const U256_0X100000000: U256 = U256([4294967296, 0, 0, 0]);
pub const U256_0X10000: U256 = U256([65536, 0, 0, 0]);
pub const U256_0X100: U256 = U256([256, 0, 0, 0]);
pub const U256_255: U256 = U256([255, 0, 0, 0]);
pub const U256_192: U256 = U256([192, 0, 0, 0]);
pub const U256_191: U256 = U256([191, 0, 0, 0]);
pub const U256_128: U256 = U256([128, 0, 0, 0]);
pub const U256_64: U256 = U256([64, 0, 0, 0]);
pub const U256_32: U256 = U256([32, 0, 0, 0]);
pub const U256_16: U256 = U256([16, 0, 0, 0]);
pub const U256_8: U256 = U256([8, 0, 0, 0]);
pub const U256_4: U256 = U256([4, 0, 0, 0]);
pub const U256_2: U256 = U256([2, 0, 0, 0]);

pub fn div_uu(x: U256, y: U256) -> Result<u128, ArithmeticError> {
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
                msb += U256::one();
            }

            answer =
                (x << (U256_255 - msb)) / (((y - U256::one()) >> (msb - U256_191)) + U256::one());
        }

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Err(ArithmeticError::ShadowOverflow(answer));
        }

        let hi = answer * (y >> U256_128);
        let mut lo = answer * (y & U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);

        let mut xh = x >> U256_192;
        let mut xl = x << U256_64;

        if xl < lo {
            xh -= U256::one();
        }

        xl = xl.overflowing_sub(lo).0;
        lo = hi << U256_128;

        if xl < lo {
            xh -= U256::one();
        }

        xl = xl.overflowing_sub(lo).0;

        if xh != hi >> U256_128 {
            return Err(ArithmeticError::RoundingError);
        }

        answer += xl / y;

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Err(ArithmeticError::ShadowOverflow(answer));
        }

        Ok(answer.as_u128())
    } else {
        Err(ArithmeticError::YIsZero)
    }
}

//Converts a Q64 fixed point to a Q16 fixed point -> f64
pub fn q64_to_f64(x: u128) -> f64 {
    BigFloat::from(x)
        .div(&BigFloat::from(U128_0X10000000000000000))
        .to_f64()
}
