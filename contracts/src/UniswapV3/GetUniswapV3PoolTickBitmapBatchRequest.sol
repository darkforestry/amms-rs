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

    struct TickBitmaps {
        int16[] wordPositions;
        uint256[] tickBitmaps;
    }

    constructor(TickBitmapInfo[] memory allPoolInfo) {
        uint256[][] memory allTickBitmaps = new uint256[][](allPoolInfo.length);

        for (uint256 i = 0; i < allPoolInfo.length; ++i) {
            TickBitmapInfo memory info = allPoolInfo[i];
            IUniswapV3PoolState pool = IUniswapV3PoolState(info.pool);

            uint256[] memory tickBitmaps = new uint256[](uint16(info.maxWord - info.minWord) + 1);

            uint256 wordIdx = 0;
            for (int16 j = info.minWord; j <= info.maxWord; ++j) {
                uint256 tickBitmap = pool.tickBitmap(j);

                if (tickBitmap == 0) {
                    continue;
                }

                tickBitmaps[wordIdx] = uint256(int256(j));
                ++wordIdx;

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
