//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract GetTokenDecimalsBatchRequest {
    constructor(address[] memory tokens) {
        uint8[] memory decimals = new uint8[](tokens.length);

        for (uint256 i = 0; i < tokens.length; ++i) {
            address token = tokens[i];

            if (codeSizeIsZero(token)) continue;

            (bool tokenDecimalsSuccess, bytes memory tokenDecimalsData) = token
                .call{gas: 20000}(abi.encodeWithSignature("decimals()"));

            if (tokenDecimalsSuccess) {
                uint256 tokenDecimals;

                if (tokenDecimalsData.length == 32) {
                    (tokenDecimals) = abi.decode(tokenDecimalsData, (uint256));

                    if (tokenDecimals == 0 || tokenDecimals > 255) {
                        continue;
                    } else {
                        decimals[i] = uint8(tokenDecimals);
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            }
        }

        bytes memory _abiEncodedData = abi.encode(decimals);
        assembly {
            // Return from the start of the data (discarding the original data address)
            // up to the end of the memory used
            let dataStart := add(_abiEncodedData, 0x20)
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
