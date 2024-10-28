//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/**
 * @dev This contract is not meant to be deployed. Instead, use a static call with the
 *       deployment bytecode as payload.
 */
contract GetUniswapV3PoolDataBatchRequest {
    struct TickBounds {
        address pool;
        int24 tickSpacing;
        int16 minWord;
        int16 maxWord;
    }

    // struct PoolData {}

    constructor(TickBounds[] memory tickBounds) {
        IUniswapV3PoolState.TickInfo[][]
            memory allTickInfo = new IUniswapV3PoolState.TickInfo[][](
                tickBounds.length
            );

        for (uint256 i = 0; i < tickBounds.length; ++i) {
            TickBounds memory tickBound = tickBounds[i];

            // Loop from min to max word inclusive and get all tick bitmaps
            IUniswapV3PoolState pool = IUniswapV3PoolState(tickBound.pool);
            for (int16 j = tickBound.minWord; j <= tickBound.maxWord; ++j) {
                uint256 tickBitmap = pool.tickBitmap(j);

                if (tickBitmap != 0) {
                    // Get all tick indices
                    int24[] memory tickIndices = new int24[](256);

                    for (uint256 k = 0; k < 256; ++k) {
                        uint256 bit = 1 << k;
                        bool initialized = (tickBitmap & bit) != 0;
                        if (initialized) {
                            tickIndices[k] =
                                int24(j * 256 + k) *
                                tickBound.tickSpacing;
                        }
                    }

                    IUniswapV3PoolState.TickInfo[]
                        memory tickInfo = new IUniswapV3PoolState.TickInfo[](
                            256
                        );

                    for (uint256 k = 0; k < 256; ++k) {
                        tickInfo[k] = pool.ticks(tickIndices[k]);
                    }
                }
            }

            // ensure abi encoding, not needed here but increase reusability for different return types
            // note: abi.encode add a first 32 bytes word with the address of the original data
            bytes memory abiEncodedData = abi.encode(tickBitmap);

            assembly {
                // Return from the start of the data (discarding the original data address)
                // up to the end of the memory used
                let dataStart := add(abiEncodedData, 0x20)
                return(dataStart, sub(msize(), dataStart))
            }
        }

        //

        // ensure abi encoding, not needed here but increase reusability for different return types
        // note: abi.encode add a first 32 bytes word with the address of the original data
        bytes memory abiEncodedData = abi.encode();

        assembly {
            // Return from the start of the data (discarding the original data address)
            // up to the end of the memory used
            let dataStart := add(abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
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
}
