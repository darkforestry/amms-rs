//SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "./WethValueInPools.sol";

contract WethValueInPoolsBatchRequest is WethValueInPools {
    constructor(
        address _uniswapV2Factory,
        address _uniswapV3Factory,
        address _weth,
        WethValueInPools.PoolInfo[] memory pools
    ) WethValueInPools(_uniswapV2Factory, _uniswapV3Factory, _weth) {
        WethValueInPools.PoolInfoReturn[] memory poolInfoReturn = getWethValueInPools(pools);
        // insure abi encoding, not needed here but increase reusability for different return types
        // note: abi.encode add a first 32 bytes word with the address of the original data
        bytes memory abiEncodedData = abi.encode(poolInfoReturn);
        assembly {
            // Return from the start of the data (discarding the original data address)
            // up to the end of the memory used
            let dataStart := add(abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }
}
