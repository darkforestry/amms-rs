//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

library FixedPointMath {
    uint256 internal constant Q96 = 0x1000000000000000000000000;

    /// @notice helper function to multiply unsigned 64.64 fixed point number by a unsigned integer
    /// @param x 64.64 unsigned fixed point number
    /// @param y uint256 unsigned integer
    /// @return unsigned
    function mul64u(uint128 x, uint256 y) internal pure returns (uint256) {
        unchecked {
            if (y == 0 || x == 0) {
                return 0;
            }

            uint256 lo = (uint256(x) *
                (y & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF)) >> 64;
            uint256 hi = uint256(x) * (y >> 128);

            if (hi > 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF) {
                return 0;
            }
            hi <<= 64;

            if (
                hi >
                0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff -
                    lo
            ) {
                return 0;
            }
            return hi + lo;
        }
    }

    /// @notice helper to divide two unsigned integers
    /// @param x uint256 unsigned integer
    /// @param y uint256 unsigned integer
    /// @return unsigned 64.64 fixed point number
    function divuu(uint256 x, uint256 y) internal pure returns (uint128) {
        unchecked {
            if (y == 0) return 0;

            uint256 answer;

            if (x <= 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF) {
                answer = (x << 64) / y;
            } else {
                uint256 msb = 192;
                uint256 xc = x >> 192;
                if (xc >= 0x100000000) {
                    xc >>= 32;
                    msb += 32;
                }
                if (xc >= 0x10000) {
                    xc >>= 16;
                    msb += 16;
                }
                if (xc >= 0x100) {
                    xc >>= 8;
                    msb += 8;
                }
                if (xc >= 0x10) {
                    xc >>= 4;
                    msb += 4;
                }
                if (xc >= 0x4) {
                    xc >>= 2;
                    msb += 2;
                }
                if (xc >= 0x2) msb += 1; // No need to shift xc anymore

                answer = (x << (255 - msb)) / (((y - 1) >> (msb - 191)) + 1);

                // require(
                //     answer <= 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF,
                //     "overflow in divuu"
                // );

                // We ignore pools that have a price that is too high because it is likely that the reserves are too low to be accurate
                // There is almost certainly not a pool that has a price of token/weth > 2^128
                if (answer > 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF) {
                    return 0;
                }

                uint256 hi = answer * (y >> 128);
                uint256 lo = answer * (y & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);

                uint256 xh = x >> 192;
                uint256 xl = x << 64;

                if (xl < lo) xh -= 1;
                xl -= lo; // We rely on overflow behavior here
                lo = hi << 128;
                if (xl < lo) xh -= 1;
                xl -= lo; // We rely on overflow behavior here

                assert(xh == hi >> 128);

                answer += xl / y;
            }

            // We ignore pools that have a price that is too high because it is likely that the reserves are too low to be accurate
            // There is almost certainly not a pool that has a price of token/weth > 2^128
            if (answer > 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF) {
                return 0;
            }

            return uint128(answer);
        }
    }

    function fromSqrtX96(
        uint160 sqrtPriceX96,
        bool token0IsReserve0,
        int8 token0Decimals,
        int8 token1Decimals
    ) internal pure returns (uint256 priceX128) {
        unchecked {
            ///@notice Cache the difference between the input and output token decimals. p=y/x ==> p*10**(x_decimals-y_decimals)>>Q192 will be the proper price in base 10.
            int8 decimalShift = token0Decimals - token1Decimals;
            ///@notice Square the sqrtPrice ratio and normalize the value based on decimalShift.
            uint256 priceSquaredX96 = decimalShift < 0
                ? uint256(sqrtPriceX96) ** 2 /
                    uint256(10) ** (uint8(-decimalShift))
                : uint256(sqrtPriceX96) ** 2 * 10 ** uint8(decimalShift);

            ///@notice The first value is a Q96 representation of p_token0, the second is 128X fixed point representation of p_token1.
            uint256 priceSquaredShiftQ96 = token0IsReserve0
                ? priceSquaredX96 / Q96
                : (Q96 * 0xffffffffffffffffffffffffffffffff) /
                    (priceSquaredX96 / Q96);

            ///@notice Convert the first value to 128X fixed point by shifting it left 128 bits and normalizing the value by Q96.
            priceX128 = token0IsReserve0
                ? (uint256(priceSquaredShiftQ96) *
                    0xffffffffffffffffffffffffffffffff) / Q96
                : priceSquaredShiftQ96;

            if (priceX128 > type(uint256).max) {
                // Essentially 0 liquidity.
                return 0;
            }
        }
    }
}
