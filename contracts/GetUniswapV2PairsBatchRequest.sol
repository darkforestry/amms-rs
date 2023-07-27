//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IFactory {
    function allPairs(uint256 idx) external returns (address);
}

/**
 @dev This contract is not meant to be deployed. Instead, use a static call with the
      deployment bytecode as payload.
 */
contract GetUniswapV2PairsBatchRequest {
    constructor(
        uint256 from,
        uint256 step,
        address factory
    ) {
        uint256 distance = step - from;

        // There is a max number of pool as a too big returned data times out the rpc
        address[] memory allPairs = new address[](distance);

        // Query every pool balance
        for (uint256 i = 0; i < distance; i++) {
            allPairs[i] = IFactory(factory).allPairs(from + i);
        }

        // ensure abi encoding, not needed here but increase reusability for different return types
        // note: abi.encode add a first 32 bytes word with the address of the original data
        bytes memory _abiEncodedData = abi.encode(allPairs);

        assembly {
            // Return from the start of the data (discarding the original data address)
            // up to the end of the memory used
            let dataStart := add(_abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }
}
