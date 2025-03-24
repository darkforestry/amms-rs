//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IBPool {
    function getCurrentTokens() external returns (address[] memory);
    function getDenormalizedWeight(address token) external returns (uint256);
    function getSwapFee() external returns (uint256);
    function getBalance(address token) external returns (uint256);
}
