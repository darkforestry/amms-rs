// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../src/UniswapV3/FixedPoint.sol";

contract FixedPointTest is Test {
    /// @dev The minimum value that can be returned from #getSqrtRatioAtTick. Equivalent to getSqrtRatioAtTick(MIN_TICK)
    uint160 internal constant MIN_SQRT_RATIO = 4295128739;
    /// @dev The maximum value that can be returned from #getSqrtRatioAtTick. Equivalent to getSqrtRatioAtTick(MAX_TICK)
    uint160 internal constant MAX_SQRT_RATIO = 1461446703485210103287273052203988822378723970342;

    function setUp() public {}

    function test_divuu_never_reverts(uint128 a, uint128 b) public pure {
        FixedPointMath.divuu(a, b);
    }

    function test_mul64u_never_reverts(uint128 a, uint256 b) public pure {
        FixedPointMath.mul64u(a, b);
    }

    function test_from_sqrt_x_96_never_reverts(
        uint160 x,
        bool token0IsReserve0,
        int8 token0Decimals,
        int8 token1Decimals
    ) public pure {
        // Bound x from min_sqrt_x_96 to max_sqrt_x_96
        if (x >= MIN_SQRT_RATIO || x <= MAX_SQRT_RATIO) {
            FixedPointMath.fromSqrtX96(x, token0IsReserve0, token0Decimals, token1Decimals);
        }
    }
}
