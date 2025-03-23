//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/**
 * @dev This contract is not meant to be deployed. Instead, use a static call with the
 *       deployment bytecode as payload.
 */
contract GetUniswapV3PoolDataBatchRequest {
    struct PoolInfo {
        address pool;
        address tokenA;
        address tokenB;
        int24 tickSpacing;
        int16 minWord;
        int16 maxWord;
    }

    struct TickInfo {
        uint128 liquidityGross;
        int128 liquidityNet;
        bool initialized;
    }

    struct PoolData {
        // NOTE: the len is from minWord to maxWord which are the keys for thehashmap
        uint256[] tickBitmap;
        int24[] tickIndices;
        TickInfo[] ticks;
    }

    constructor(PoolInfo[] memory poolInfo) {
        PoolData[] memory allPoolData = new PoolData[](poolInfo.length);

        for (uint256 i = 0; i < poolInfo.length; ++i) {
            PoolInfo memory info = poolInfo[i];
            IUniswapV3PoolState pool = IUniswapV3PoolState(info.pool);

            PoolData memory poolData = allPoolData[i];
            uint256 wordRange = uint256(int256(info.maxWord - info.minWord)) + 1;

            poolData.tickBitmap = new uint256[](wordRange);

            TickInfo[] memory tickInfo = new TickInfo[](256 * wordRange);
            int24[] memory tickIdxs = new int24[](256 * wordRange);

            uint256 tickArrayIndex = 0;

            // Loop from min to max word inclusive and get all tick bitmaps

            // NOTE: since we are iterating over this range and
            // getting the the tick index accordingly this will overflow
            uint256 wordRangeIdx = 0;
            for (int16 j = info.minWord; j <= info.maxWord; ++j) {
                uint256 tickBitmap = pool.tickBitmap(j);

                if (tickBitmap == 0) {
                    continue;
                }

                for (uint256 k = 0; k < 256; ++k) {
                    uint256 bit = 1 << k;

                    bool initialized = (tickBitmap & bit) != 0;
                    if (initialized) {
                        int24 tickIndex = int24(int256(wordRangeIdx * 256 + k * uint256(int256(info.tickSpacing))));

                        IUniswapV3PoolState.TickInfo memory tick = pool.ticks(tickIndex);

                        tickIdxs[tickArrayIndex] = tickIndex;
                        tickInfo[tickArrayIndex] = TickInfo({
                            liquidityGross: tick.liquidityGross,
                            liquidityNet: tick.liquidityNet,
                            initialized: tick.initialized
                        });

                        ++tickArrayIndex;
                    }
                }

                poolData.tickBitmap[wordRangeIdx] = tickBitmap;
                ++wordRangeIdx;
            }

            assembly {
                mstore(tickInfo, tickArrayIndex)
                mstore(tickIdxs, tickArrayIndex)
            }

            poolData.ticks = tickInfo;
            poolData.tickIndices = tickIdxs;
            allPoolData[i] = poolData;
        }

        // ensure abi encoding, not needed here but increase reusability for different return types
        // note: abi.encode add a first 32 bytes word with the address of the original data
        bytes memory abiEncodedData = abi.encode(allPoolData);

        assembly {
            // Return from the start of the data (discarding the original data address)
            // up to the end of the memory used
            let dataStart := add(abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }
}

function codeSizeIsZero(address target) view returns (bool) {
    if (target.code.length == 0) {
        return true;
    } else {
        return false;
    }
}

/// @title Pool state that can change
/// @notice These methods compose the pool's state, and can change with any frequency including multiple times
/// per transaction
interface IUniswapV3PoolState {
    struct TickInfo {
        // the total position liquidity that references this tick
        uint128 liquidityGross;
        // amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left),
        int128 liquidityNet;
        // fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
        // only has relative meaning, not absolute — the value depends on when the tick is initialized
        uint256 feeGrowthOutside0X128;
        uint256 feeGrowthOutside1X128;
        // the cumulative tick value on the other side of the tick
        int56 tickCumulativeOutside;
        // the seconds per unit of liquidity on the _other_ side of this tick (relative to the current tick)
        // only has relative meaning, not absolute — the value depends on when the tick is initialized
        uint160 secondsPerLiquidityOutsideX128;
        // the seconds spent on the other side of the tick (relative to the current tick)
        // only has relative meaning, not absolute — the value depends on when the tick is initialized
        uint32 secondsOutside;
        // true iff the tick is initialized, i.e. the value is exactly equivalent to the expression liquidityGross != 0
        // these 8 bits are set to prevent fresh sstores when crossing newly initialized ticks
        bool initialized;
    }

    function ticks(int24 tick) external view returns (TickInfo memory);

    /// @notice Returns 256 packed tick initialized boolean values. See TickBitmap for more information
    function tickBitmap(int16 wordPosition) external view returns (uint256);

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
