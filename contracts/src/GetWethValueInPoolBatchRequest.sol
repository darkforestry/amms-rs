// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

contract GetWethValueInPoolBatchRequest {
    uint256 internal constant Q96 = 0x1000000000000000000000000;
    address internal constant ADDRESS_ZERO = address(0);

    mapping(address => uint128) public tokenToWethPrices;

    constructor(
        address[] memory pools,
        address[] memory dexes,
        bool[] memory dexIsUniV3,
        address weth,
        uint256 wethInPoolThreshold
    ) {
        uint256[] memory wethValueInPools = new uint256[](pools.length);

        for (uint256 i = 0; i < pools.length; ++i) {
            if (badPool(pools[i])) {
                wethValueInPools[i] = 0;
                continue;
            }

            // Get the token0 and token1 from the pool
            if (!codeSizeIsZero(pools[i])) {
                address token0 = IUniswapV2Pair(pools[i]).token0();
                address token1 = IUniswapV2Pair(pools[i]).token1();

                if (!codeSizeIsZero(token0) && !codeSizeIsZero(token1)) {
                    //Get the reserves from the pool

                    (uint256 r0, uint256 r1) = getNormalizedReserves(pools[i], token0, token1);

                    //Get the value of the tokens in the pool in weth
                    uint256 token0WethValueInPool =
                        getWethValueOfToken(token0, weth, r0, dexes, dexIsUniV3, wethInPoolThreshold);

                    uint256 token1WethValueInPool =
                        getWethValueOfToken(token1, weth, r1, dexes, dexIsUniV3, wethInPoolThreshold);

                    if (token0WethValueInPool != 0 && token1WethValueInPool != 0) {
                        // add the aggregate weth value of both of the tokens in the pool to the wethValueInPools array
                        wethValueInPools[i] = token0WethValueInPool + token1WethValueInPool;
                    } else {
                        wethValueInPools[i] = 0;
                    }
                } else {
                    wethValueInPools[i] = 0;
                }
            } else {
                wethValueInPools[i] = 0;
            }
        }

        // insure abi encoding, not needed here but increase reusability for different return types
        // note: abi.encode add a first 32 bytes word with the address of the original data
        bytes memory abiEncodedData = abi.encode(wethValueInPools);

        assembly {
            // Return from the start of the data (discarding the original data address)
            // up to the end of the memory used
            let dataStart := add(abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }

    function badPool(address lp) internal returns (bool) {
        //If the pool is v3
        if (!lpIsNotUniV3(lp)) {
            if (IUniswapV3PoolState(lp).liquidity() == 0) {
                return true;
            }
        }

        return false;
    }

    function getWethValueOfToken(
        address token,
        address weth,
        uint256 amount,
        address[] memory dexes,
        bool[] memory dexIsUniV3,
        uint256 wethInPoolThreshold
    ) internal returns (uint256) {
        //If the token is weth, the amount is the amount of weth in the pool for that token
        //Note: We return the normalized amount of weth, which is 18 decimals. If we ever change our normalization logic,
        //we need to account for this as we are returning the normalized amount
        if (token == weth) {
            return amount;
        }

        uint128 tokenToWethPrice = tokenToWethPrices[token];

        //If the price is 1, that means that the price has already been marked as invalid, so we return zero
        if (tokenToWethPrice == 1) {
            return 0;
        } else {
            //If the price is not 0, that means that we have already calculated the price and we can apply it
            if (tokenToWethPrice != 0) {
                return mul64u(tokenToWethPrice, amount);
            } else {
                //Otherwise, we either apply the price  or we get the price and then derive the token to weth value in the pool
                for (uint256 i = 0; i < dexes.length; ++i) {
                    uint128 price = getTokenToWethPrice(token, weth, dexes[i], dexIsUniV3[i], wethInPoolThreshold);

                    if (price != 0) {
                        return mul64u(price, amount);
                    }
                }
            }

            //If no dexes have a valid price for the token, return 0
            return 0;
        }
    }

    function getTokenToWethPrice(
        address token,
        address weth,
        address dexFactory,
        bool isUniV3,
        uint256 wethInPoolThreshold
    ) internal returns (uint128) {
        if (isUniV3) {
            uint16[3] memory feeTiers = [500, 3000, 10000];
            for (uint256 i = 0; i < feeTiers.length; ++i) {
                address pairAddress = IUniswapV3Factory(dexFactory).getPool(
                    token < weth ? token : weth, token < weth ? weth : token, feeTiers[i]
                );

                if (pairAddress != ADDRESS_ZERO) {
                    ///Check here if the weth in pool threshold is met
                    uint128 price = getTokenToWethPriceFromPool(isUniV3, token, weth, pairAddress, wethInPoolThreshold);

                    if (price != 0) {
                        return price;
                    }
                }
            }
        } else {
            bool tokenIsToken0 = token < weth;

            address pairAddress =
                IUniswapV2Factory(dexFactory).getPair(tokenIsToken0 ? token : weth, tokenIsToken0 ? weth : token);

            if (pairAddress != ADDRESS_ZERO) {
                uint128 price = getTokenToWethPriceFromPool(isUniV3, token, weth, pairAddress, wethInPoolThreshold);

                if (price != 0) {
                    return price;
                }
            }
        }

        //We set the price to 1 so that we know that the token to weth pairing does not exist or is not valid
        tokenToWethPrices[token] = 1;
        return 0;
    }

    function getTokenToWethPriceFromPool(
        bool isUniV3,
        address token,
        address weth,
        address pool,
        uint256 wethLiquidityThreshold
    ) internal returns (uint128 price) {
        bool tokenIsToken0 = token < weth;
        if (!isUniV3) {
            (uint256 r_0, uint256 r_1) = getNormalizedReserves(pool, token, weth);

            //Check if the weth value meets the threshold
            //Note: Normalization normalizes the decimals to 18 decimals. If there is ever a weth value that does not have 18 decimals for the chain
            //or we change our normalization logic, we need to account for this
            if (tokenIsToken0) {
                if (r_1 < wethLiquidityThreshold) {
                    return 0;
                }
            } else {
                if (r_0 < wethLiquidityThreshold) {
                    return 0;
                }
            }

            price = divuu(tokenIsToken0 ? r_1 : r_0, tokenIsToken0 ? r_0 : r_1);
        } else {
            (uint160 sqrtPriceX96,,,,,,) = IUniswapV3PoolState(pool).slot0();
            price = uint128(fromSqrtX96(sqrtPriceX96, tokenIsToken0, token, weth) >> 64);
        }

        //Add the price to the tokenToWeth price mapping
        tokenToWethPrices[token] = price;
    }

    function getReserves(address lp, address token0, address token1) internal returns (uint256, uint256) {
        (token0, token1) = (token0 < token1) ? (token0, token1) : (token1, token0);

        uint256 r_x;
        uint256 r_y;

        if (lpIsNotUniV3(lp)) {
            (uint112 r_x_112, uint112 r_y_112,) = IUniswapV2Pair(lp).getReserves();
            r_x = r_x_112;
            r_y = r_y_112;
        } else {
            (uint256 lpBalanceOfToken0, bool success0) = getBalanceOfUnsafe(token0, lp);
            (uint256 lpBalanceOfToken1, bool success1) = getBalanceOfUnsafe(token1, lp);

            if (success0 && success1) {
                if (token0 < token1) {
                    r_x = lpBalanceOfToken0;
                    r_y = lpBalanceOfToken1;
                } else {
                    r_y = lpBalanceOfToken0;
                    r_x = lpBalanceOfToken1;
                }
            }
        }

        return (r_x, r_y);
    }

    function getNormalizedReserves(address lp, address token0, address token1) internal returns (uint256, uint256) {
        (uint256 r_x, uint256 r_y) = getReserves(lp, token0, token1);

        return normalizeReserves(r_x, r_y, token0 < token1 ? token0 : token1, token0 < token1 ? token1 : token0);
    }

    function normalizeReserves(uint256 x, uint256 y, address token0, address token1)
        internal
        returns (uint256 r_x, uint256 r_y)
    {
        (uint8 token0Decimals, bool t0s) = getTokenDecimalsUnsafe(token0);
        (uint8 token1Decimals, bool t1s) = getTokenDecimalsUnsafe(token1);

        if (t0s && t1s) {
            r_x = token0Decimals <= 18 ? x * (10 ** (18 - token0Decimals)) : x / (10 ** (token0Decimals - 18));
            r_y = token1Decimals <= 18 ? y * (10 ** (18 - token1Decimals)) : y / (10 ** (token1Decimals - 18));
        }
    }

    function fromSqrtX96(uint160 sqrtPriceX96, bool token0IsReserve0, address token0, address token1)
        internal
        returns (uint256 priceX128)
    {
        unchecked {
            ///@notice very weird edge case
            if (sqrtPriceX96 == 0) {
                return 0;
            }

            (uint8 token0Decimals, bool t0s) = getTokenDecimalsUnsafe(token0);
            (uint8 token1Decimals, bool t1s) = getTokenDecimalsUnsafe(token1);

            ///@notice if either of tokens does not implement `decimals()` return price as 0
            if (!(t0s && t1s)) {
                return 0;
            }

            ///@notice Cache the difference between the input and output token decimals. p=y/x ==> p*10**(x_decimals-y_decimals)>>Q192 will be the proper price in base 10.
            int8 decimalShift = int8(token0Decimals) - int8(token1Decimals);
            ///@notice Square the sqrtPrice ratio and normalize the value based on decimalShift.
            uint256 priceSquaredX96 = decimalShift < 0
                ? uint256(sqrtPriceX96) ** 2 / uint256(10) ** (uint8(-decimalShift))
                : uint256(sqrtPriceX96) ** 2 * 10 ** uint8(decimalShift);

            ///@notice The first value is a Q96 representation of p_token0, the second is 128X fixed point representation of p_token1.
            uint256 priceSquaredShiftQ96 =
                token0IsReserve0 ? priceSquaredX96 << 128 / Q96 : type(uint256).max / (priceSquaredX96 << 128 / Q96);

            ///@notice Convert the first value to 128X fixed point by shifting it left 128 bits and normalizing the value by Q96.
            priceX128 = token0IsReserve0 ? priceSquaredShiftQ96 / Q96 : priceSquaredShiftQ96;
            require(priceX128 <= type(uint256).max, "Overflow");
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

    /// @notice returns true as the second return value if the token decimals can be successfully retrieved
    function getTokenDecimalsUnsafe(address token) internal returns (uint8, bool) {
        (bool tokenDecimalsSuccess, bytes memory tokenDecimalsData) =
            token.call{gas: 20000}(abi.encodeWithSignature("decimals()"));

        if (tokenDecimalsSuccess) {
            uint256 tokenDecimals;

            if (tokenDecimalsData.length == 32) {
                (tokenDecimals) = abi.decode(tokenDecimalsData, (uint256));

                if (tokenDecimals == 0 || tokenDecimals > 255) {
                    return (0, false);
                } else {
                    return (uint8(tokenDecimals), true);
                }
            } else {
                return (0, false);
            }
        } else {
            return (0, false);
        }
    }

    /// @notice returns true as the second return value if the token decimals can be successfully retrieved
    function getBalanceOfUnsafe(address token, address targetAddress) internal returns (uint256, bool) {
        (bool balanceOfSuccess, bytes memory balanceOfData) =
            token.call(abi.encodeWithSignature("balanceOf(address)", targetAddress));

        if (balanceOfSuccess) {
            uint256 balance;

            if (balanceOfData.length == 32) {
                (balance) = abi.decode(balanceOfData, (uint256));

                return (balance, true);
            } else {
                return (0, false);
            }
        } else {
            return (0, false);
        }
    }

    /// @notice helper function to multiply unsigned 64.64 fixed point number by a unsigned integer
    /// @param x 64.64 unsigned fixed point number
    /// @param y uint256 unsigned integer
    /// @return unsigned
    function mul64u(uint128 x, uint256 y) internal pure returns (uint256) {
        unchecked {
            if (y == 0 || x == 0) {
                return 0;
            }

            uint256 lo = (uint256(x) * (y & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF)) >> 64;
            uint256 hi = uint256(x) * (y >> 128);

            require(hi <= 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF, "overflow-0 in mul64u");
            hi <<= 64;

            require(
                hi <= 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff - lo, "overflow-1 in mul64u"
            );
            return hi + lo;
        }
    }

    ///@notice Helper function to determine if a pool address is Uni V2 compatible.
    ///@param lp - Pair address.
    ///@return bool Indicator whether the pool is not Uni V3 compatible.
    function lpIsNotUniV3(address lp) internal returns (bool) {
        bool success;
        assembly {
            //store the function sig for  "fee()"
            mstore(0x00, 0xddca3f4300000000000000000000000000000000000000000000000000000000)

            success :=
                call(
                    gas(), // gas remaining
                    lp, // destination address
                    0, // no ether
                    0x00, // input buffer (starts after the first 32 bytes in the `data` array)
                    0x04, // input length (loaded from the first 32 bytes in the `data` array)
                    0x00, // output buffer
                    0x00 // output length
                )
        }
        ///@notice return the opposite of success, meaning if the call succeeded, the address is univ3, and we should
        ///@notice indicate that lpIsNotUniV3 is false
        return !success;
    }

    function codeSizeIsZero(address target) internal view returns (bool) {
        if (target.code.length == 0) {
            return true;
        } else {
            return false;
        }
    }
}

//=======================================
// Interfaces
//Note: Just flattening this to keep everything in one place for the batch contract
//=======================================

interface IUniswapV3Factory {
    function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
}

interface IERC20 {
    function decimals() external view returns (uint8);

    function balanceOf(address account) external view returns (uint256);
}

interface IUniswapV2Factory {
    function getPair(address tokenA, address tokenB) external view returns (address pair);
}

interface IUniswapV2Pair {
    function decimals() external pure returns (uint8);

    function token0() external view returns (address);

    function token1() external view returns (address);

    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
}

interface IUniswapV3PoolState {
    function slot0()
        external
        view
        returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );

    function liquidity() external view returns (uint128);
}
