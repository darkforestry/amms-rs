// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../src/WethValueInPools.sol";

contract WethValueInPoolsTest is Test {
    WethValueInPools public wethValueInPools;
    address uniswapV2Factory = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;
    address uniswapV3Factory = 0x1F98431c8aD98523631AE4a59f267346ea31F984;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;

    function setUp() public {
        wethValueInPools = new WethValueInPools(
            uniswapV2Factory,
            uniswapV3Factory,
            WETH
        );
    }

    function test_getWethValueInPools_validWeth() public {
        WethValueInPools.PoolInfo[]
            memory testFixtureValidWeth = new WethValueInPools.PoolInfo[](3);
        testFixtureValidWeth[0] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.Balancer,
            poolAddress: 0x8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf
        });
        testFixtureValidWeth[1] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.UniswapV2,
            poolAddress: 0x397FF1542f962076d0BFE58eA045FfA2d347ACa0
        });
        testFixtureValidWeth[2] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.UniswapV3,
            poolAddress: 0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640
        });
        WethValueInPools.PoolInfoReturn[] memory pools = wethValueInPools
            .getWethValueInPools(testFixtureValidWeth);
        assertEq(pools.length, 3);
        // Check weth value > 0
        assertGt(pools[0].wethValue, 0);
        assertGt(pools[1].wethValue, 0);
        assertGt(pools[2].wethValue, 0);
    }

    function test_getWethValueInPools_validNoWeth() public {
        WethValueInPools.PoolInfo[]
            memory testFixtureValidNoWeth = new WethValueInPools.PoolInfo[](3);
        testFixtureValidNoWeth[0] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.Balancer,
            poolAddress: 0xE5D1fAB0C5596ef846DCC0958d6D0b20E1Ec4498
        });
        testFixtureValidNoWeth[1] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.UniswapV2,
            poolAddress: 0xAE461cA67B15dc8dc81CE7615e0320dA1A9aB8D5
        });
        testFixtureValidNoWeth[2] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.UniswapV3,
            poolAddress: 0x6c6Bc977E13Df9b0de53b251522280BB72383700
        });
        WethValueInPools.PoolInfoReturn[] memory pools = wethValueInPools
            .getWethValueInPools(testFixtureValidNoWeth);
        assertEq(pools.length, 3);
        // Check weth value > 0
        assertGt(pools[0].wethValue, 0);
        assertGt(pools[1].wethValue, 0);
        assertGt(pools[2].wethValue, 0);
    }

    function test_getWethValueInPools_invalid_no_revert() public {
        WethValueInPools.PoolInfo[]
            memory testFixtureInvalid = new WethValueInPools.PoolInfo[](3);
        testFixtureInvalid[0] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.Balancer,
            poolAddress: 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f
        });
        testFixtureInvalid[1] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.UniswapV2,
            poolAddress: 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f
        });
        testFixtureInvalid[2] = WethValueInPools.PoolInfo({
            poolType: WethValueInPools.PoolType.UniswapV3,
            poolAddress: 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f
        });
        WethValueInPools.PoolInfoReturn[] memory pools = wethValueInPools
            .getWethValueInPools(testFixtureInvalid);
        assertEq(pools.length, 3);
        // Should all be zero
        assertEq(pools[0].wethValue, 0);
        assertEq(pools[1].wethValue, 0);
        assertEq(pools[2].wethValue, 0);
    }
}
