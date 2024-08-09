//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IBPool {
    function getCurrentTokens() external returns (address[] memory);
    function getDenormalizedWeight(address token) external returns (uint);
    function getSwapFee() external returns (uint);
}

interface IERC20 {
    function decimals() external view returns (uint8);
}

/**
 * @dev This contract is not meant to be deployed. Instead, use a static call with the
 *       deployment bytecode as payload.
 */
contract GetBalancerV2PoolDataBatchRequest {
    struct PoolData {
        address[] tokens;
        uint8[] decimals;
        uint256[] liquidity;
        uint32 fee;
    }

    constructor(address[] memory pools) {
        PoolData[] memory allPoolData = new PoolData[](pools.length);

        for (uint256 i = 0; i < pools.length; ++i) {
            address poolAddress = pools[i];

            if (codeSizeIsZero(poolAddress)) continue;

            PoolData memory poolData;

            // Get the tokens
            address[] memory tokens = IBPool(poolAddress).getCurrentTokens();
            for (uint256 j = 0; j < tokens.length; ++j) {
                if (!(codeSizeIsZero(tokens[j]))) {
                    poolData.tokens[j] = tokens[j];
                } else {
                    continue;
                }
            }

            // Grab the decimals/liquidity
            for (uint256 j = 0; j < poolData.tokens.length; ++j) {
                uint8 tokenDecimals = getTokenDecimals(poolData.tokens[j]);
                if (tokenDecimals == 0) {
                    continue;
                } else {
                    poolData.decimals[j] = tokenDecimals;
                }
                poolData.liquidity[j] = IBPool(poolAddress).getDenormalizedWeight(
                    poolData.tokens[j]
                );
            }

            // Grab the swap fee
            poolData.fee = uint32(IBPool(poolAddress).getSwapFee());
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

    function getTokenDecimals(address token) internal returns (uint8) {
        (bool success, bytes memory data) = token.call(
            abi.encodeWithSignature("decimals()")
        );

        if (success) {
            uint256 decimals;
            if (data.length == 32) {
                (decimals) = abi.decode(data, (uint256));
                if (decimals == 0 || decimals > 255) {
                    return 0;
                } else {
                    return uint8(decimals);
                }
            } else {
                return 0;
            }
        } else {
            return 0;
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
