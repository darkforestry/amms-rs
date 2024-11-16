//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/**
 * @dev This contract is not meant to be deployed. Instead, use a static call with the
 *       deployment bytecode as payload.
 */

contract GetUniswapV3PoolTickBitmapBatchRequest {
    struct TickBitmapInfo {
        address pool;
        int16 minWord;
        int16 maxWord;
    }

    /// @notice TODO: add comments about encoding scheme

    constructor(TickBitmapInfo[] memory allPoolInfo) {
        uint256[][] memory allTickBitmaps = new uint256[][](allPoolInfo.length);

        for (uint256 i = 0; i < allPoolInfo.length; ++i) {
            TickBitmapInfo memory info = allPoolInfo[i];
            IUniswapV3PoolState pool = IUniswapV3PoolState(info.pool);

            uint256[] memory tickBitmaps = new uint256[](
                uint16(info.maxWord - info.minWord) + 1
            );

            uint256 wordIdx = 0;
            for (int16 j = info.minWord; j <= info.maxWord; ++j) {
                uint256 tickBitmap = pool.tickBitmap(j);

                if (tickBitmap == 0) {
                    continue;
                }

                /// @notice We pack the word position within the tickSpacing of the tickBitmap
                /// to reduce the size of deployed bytecode which enables larger batch calls and faster sync times
                uint256 tickSpacing = uint24(pool.tickSpacing());

                // If tick spacing can fit the entire wordPos in a single contiguous string of bytes
                // left shift wordPos to fit in this spacing and add to tick bitmap
                if (tickSpacing > 16) {
                    tickBitmap += j << (255 - tickSpacing);
                } else {
                    // If the wordPos can not fit into a single tickSpacing, we must break up the tick spacing over
                    // subsequent tick spacings
                    uint256 numGroups = (16 % tickSpacing == 0)
                        ? 16 / tickSpacing
                        : (16 / tickSpacing) + 1;

                    uint256 mask = type(uint16).max << (16 - tickSpacing);

                    for (i = 0; i <= numGroups; ++i) {
                        uint256 bits = uint256(j) & (mask >> (i * tickSpacing));
                        tickBitmap += (bits << (255 - (i + 1) * tickSpacing));
                    }
                }

                tickBitmaps[wordIdx] = tickBitmap;

                ++wordIdx;
            }

            assembly {
                mstore(tickBitmaps, wordIdx)
            }

            allTickBitmaps[i] = tickBitmaps;
        }

        // ensure abi encoding, not needed here but increase reusability for different return types
        // note: abi.encode add a first 32 bytes word with the address of the original data
        bytes memory abiEncodedData = abi.encode(allTickBitmaps);

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
    /// @notice Returns 256 packed tick initialized boolean values. See TickBitmap for more information
    function tickBitmap(int16 wordPosition) external view returns (uint256);
    function tickSpacing() external view returns (int24);
}
