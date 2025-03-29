// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../src/filters/WethValueInPoolsBatchRequest.sol";

contract WethValueInPoolsBatchRequestTest is Test {
    address uniswapV2Factory = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;
    address uniswapV3Factory = 0x1F98431c8aD98523631AE4a59f267346ea31F984;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;

    function setUp() public {}

    function test_WethValueInPoolsBatchRequest_validWeth() public {
        WethValueInPools.PoolInfo[] memory testFixtureValidWeth = new WethValueInPools.PoolInfo[](3);
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

        bytes memory returnData = address(
            new WethValueInPoolsBatchRequest(uniswapV2Factory, uniswapV3Factory, WETH, testFixtureValidWeth)
        ).code;
        WethValueInPools.PoolInfoReturn[] memory pools = abi.decode(returnData, (WethValueInPools.PoolInfoReturn[]));

        assertEq(pools.length, 3);
        // Check weth value > 0 and valid pool address and pool type
        for (uint256 i = 0; i < pools.length; i++) {
            assertGt(pools[i].wethValue, 0);
            assertEq(uint8(pools[i].poolType), uint8(testFixtureValidWeth[i].poolType));
            assertEq(pools[i].poolAddress, testFixtureValidWeth[i].poolAddress);
        }
    }

    function test_WethValueInPoolsBatchRequest_validNoWeth() public {
        WethValueInPools.PoolInfo[] memory testFixtureValidNoWeth = new WethValueInPools.PoolInfo[](3);
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

        bytes memory returnData = address(
            new WethValueInPoolsBatchRequest(uniswapV2Factory, uniswapV3Factory, WETH, testFixtureValidNoWeth)
        ).code;
        WethValueInPools.PoolInfoReturn[] memory pools = abi.decode(returnData, (WethValueInPools.PoolInfoReturn[]));

        assertEq(pools.length, 3);
        // Check weth value > 0 and valid pool address and pool type
        for (uint256 i = 0; i < pools.length; i++) {
            assertGt(pools[i].wethValue, 0);
            assertEq(uint8(pools[i].poolType), uint8(testFixtureValidNoWeth[i].poolType));
            assertEq(pools[i].poolAddress, testFixtureValidNoWeth[i].poolAddress);
        }
    }

    function test_WethValueInPoolsBatchRequest_invalid_no_revert() public {
        WethValueInPools.PoolInfo[] memory testFixtureInvalid = new WethValueInPools.PoolInfo[](3);
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

        bytes memory returnData =
            address(new WethValueInPoolsBatchRequest(uniswapV2Factory, uniswapV3Factory, WETH, testFixtureInvalid)).code;
        WethValueInPools.PoolInfoReturn[] memory pools = abi.decode(returnData, (WethValueInPools.PoolInfoReturn[]));

        assertEq(pools.length, 3);
        // All weth values should be zero
        for (uint256 i = 0; i < pools.length; i++) {
            assertEq(pools[i].wethValue, 0);
            assertEq(uint8(pools[i].poolType), uint8(testFixtureInvalid[i].poolType));
            assertEq(pools[i].poolAddress, testFixtureInvalid[i].poolAddress);
        }
    }
}
