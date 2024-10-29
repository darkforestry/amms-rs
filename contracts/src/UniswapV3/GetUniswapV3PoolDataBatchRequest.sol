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

    struct PoolData {
        uint8 tokenADecimals;
        uint8 tokenBDecimals;
        int24 tick;
        uint256 liquidity;
        uint256 sqrtPrice;
        // NOTE: the len is from minWord to maxWord which are the keys for thehashmap
        uint256[] tickBitmap;
        int24[] tickIndices;
        IUniswapV3PoolState.TickInfo[] ticks;
    }

    constructor(PoolInfo[] memory poolInfo) {
        PoolData[] memory allPoolData = new PoolData[](poolInfo.length);

        for (uint256 i = 0; i < poolInfo.length; ++i) {
            PoolInfo memory info = poolInfo[i];

            // Check that tokenA and tokenB do not have codesize of 0
            if (codeSizeIsZero(info.tokenA)) continue;
            if (codeSizeIsZero(info.tokenB)) continue;

            PoolData memory poolData = allPoolData[i];

            // Get tokenA decimals
            (bool tokenADecimalsSuccess, bytes memory tokenADecimalsData) = info
                .tokenA
                .call{gas: 20000}(abi.encodeWithSignature("decimals()"));

            if (tokenADecimalsSuccess) {
                uint256 tokenADecimals;

                if (tokenADecimalsData.length == 32) {
                    (tokenADecimals) = abi.decode(
                        tokenADecimalsData,
                        (uint256)
                    );

                    if (tokenADecimals == 0 || tokenADecimals > 255) {
                        continue;
                    } else {
                        poolData.tokenADecimals = uint8(tokenADecimals);
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            }

            // Get tokenB decimals
            (bool tokenBDecimalsSuccess, bytes memory tokenBDecimalsData) = info
                .tokenB
                .call{gas: 20000}(abi.encodeWithSignature("decimals()"));

            if (tokenBDecimalsSuccess) {
                uint256 tokenBDecimals;

                if (tokenBDecimalsData.length == 32) {
                    (tokenBDecimals) = abi.decode(
                        tokenBDecimalsData,
                        (uint256)
                    );

                    if (tokenBDecimals == 0 || tokenBDecimals > 255) {
                        continue;
                    } else {
                        poolData.tokenBDecimals = uint8(tokenBDecimals);
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            }

            IUniswapV3PoolState pool = IUniswapV3PoolState(info.pool);
            poolData.liquidity = pool.liquidity();
            (poolData.sqrtPrice, poolData.tick, , , , , ) = pool.slot0();
            IUniswapV3PoolState.TickInfo[]
                memory tickInfo = new IUniswapV3PoolState.TickInfo[](
                    256 * uint16((info.maxWord - info.minWord + 1))
                );
            int24[] memory tickIdxs = new int24[](
                256 * uint16((info.maxWord - info.minWord + 1))
            );

            // Loop from min to max word inclusive and get all tick bitmaps
            for (int16 j = info.minWord; j <= info.maxWord; ++j) {
                uint256 tickBitmap = pool.tickBitmap(j);

                if (tickBitmap != 0) {
                    // Get all tick indices
                    int24[] memory tickIndices = new int24[](256);

                    for (uint256 k = 0; k < 256; ++k) {
                        uint256 bit = 1 << k;
                        bool initialized = (tickBitmap & bit) != 0;
                        if (initialized) {
                            tickIndices[k] =
                                int24(uint16(j) * 256 + uint24(k)) *
                                info.tickSpacing;

                            tickIdxs[uint16(j) * 256 + k] = tickIndices[k];
                        }
                    }

                    for (uint256 k = 0; k < 256; ++k) {
                        tickInfo[uint16(j) * 256 + k] = pool.ticks(
                            tickIndices[k]
                        );
                    }

                    poolData.tickBitmap[i] = tickBitmap;
                }
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

    function codeSizeIsZero(address target) internal view returns (bool) {
        if (target.code.length == 0) {
            return true;
        } else {
            return false;
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
