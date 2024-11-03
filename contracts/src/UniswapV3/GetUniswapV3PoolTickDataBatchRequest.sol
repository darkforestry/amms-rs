//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/**
 * @dev This contract is not meant to be deployed. Instead, use a static call with the
 *       deployment bytecode as payload.
 */

contract GetUniswapV3PoolTickDataBatchRequest {
    struct TickDataInfo {
        address pool;
        int24[] ticks;
    }

    // NOTE: we can update the return type to be more specific and reduce size
    // TODO: pick a bettter name for this
    struct Info {
        uint128 liquidityGross;
        int128 liquidityNet;
        bool initialized;
    }

    constructor(TickDataInfo[] memory allPoolInfo) {
        Info[][] memory tickInfoReturn = new Info[][](allPoolInfo.length);

        for (uint256 i = 0; i < allPoolInfo.length; ++i) {
            Info[] memory tickInfo = new Info[](allPoolInfo[i].ticks.length);
            for (uint256 j = 0; j < allPoolInfo[i].ticks.length; ++j) {
                IUniswapV3PoolState.Info memory tick = IUniswapV3PoolState(
                    allPoolInfo[i].pool
                ).ticks(allPoolInfo[i].ticks[j]);

                tickInfo[j] = Info({
                    liquidityGross: tick.liquidityGross,
                    liquidityNet: tick.liquidityNet,
                    initialized: tick.initialized
                });
            }
            tickInfoReturn[i] = tickInfo;
        }

        // ensure abi encoding, not needed here but increase reusability for different return types
        // note: abi.encode add a first 32 bytes word with the address of the original data
        bytes memory abiEncodedData = abi.encode(tickInfoReturn);

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
    struct Info {
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

    /// @notice Returns 256 packed tick initialized boolean values. See TickBitmap for more information
    function tickBitmap(int16 wordPosition) external view returns (uint256);
    function ticks(int24 tick) external view returns (Info memory);
}
