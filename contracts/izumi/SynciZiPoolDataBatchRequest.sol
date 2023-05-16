//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IiZiSwapPool {
    function liquidity(bytes32 key)
        external
        view
        returns (
            uint128 liquidity,
            uint256 lastFeeScaleX_128,
            uint256 lastFeeScaleY_128,
            uint256 tokenOwedX,
            uint256 tokenOwedY
        );
    
    function tokenX() external view returns (address);
    function tokenY() external view returns (address);

    function sqrtRate_96() external view returns(uint160);
    function fee() external view returns (uint24);
    function pointDelta() external view returns (int24);
    function state()
        external view
        returns(
            uint160 sqrtPrice_96,
            int24 currentPoint,
            uint16 observationCurrentIndex,
            uint16 observationQueueLen,
            uint16 observationNextQueueLen,
            bool locked,
            uint128 liquidity,
            uint128 liquidityX
        );
}

/**
 @dev This contract is not meant to be deployed. Instead, use a static call with the
      deployment bytecode as payload.
 */
contract GetUniswapV3PoolDataBatchRequest {
    struct PoolData {
        uint128 liquidity;
        uint160 sqrtPrice;
        uint128 liquidityA;
        uint128 liquidityB;
        int24 currentPoint;
    }

    constructor(address[] memory pools) {
        PoolData[] memory allPoolData = new PoolData[](pools.length);

        for (uint256 i = 0; i < pools.length; ++i) {
            address poolAddress = pools[i];

            if (codeSizeIsZero(poolAddress)) continue;

            PoolData memory poolData;

            (
                uint160 sqrtPriceX96,
                int24 currentPoint,
                ,
                ,
                ,
                ,
                uint128 liquidity,
                uint128 liquidityX
            ) = IiZiSwapPool(poolAddress).state();

            poolData.sqrtPrice = sqrtPriceX96;
            poolData.currentPoint = currentPoint;
            poolData.liquidity = liquidity;
            poolData.liquidityA = liquidityX;
            poolData.liquidityB = liquidity - liquidityX;
            allPoolData[i] = poolData;
        }

        bytes memory _abiEncodedData = abi.encode(allPoolData);
        assembly {
            // Return from the start of the data (discarding the original data address)
            // up to the end of the memory used
            let dataStart := add(_abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }

    function codeSizeIsZero(address target) internal view returns (bool) {
        if (target.code.length == 0) {
            return true;
        } else {
            return false;
        }
    }
}
