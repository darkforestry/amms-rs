//SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IERC4626Vault {
    function asset() external view returns (address);

    function decimals() external view returns (uint8);

    function totalSupply() external view returns (uint256);

    function totalAssets() external view returns (uint256);

    function convertToShares(uint256 assets) external view returns (uint256);

    function convertToAssets(uint256 shares) external view returns (uint256);

    function previewDeposit(uint256 assets) external view returns (uint256);

    function previewRedeem(uint256 shares) external view returns (uint256);
}

interface IERC20 {
    function decimals() external view returns (uint8);
}

/**
 * @dev This contract is not meant to be deployed. Instead, use a static call with the
 *       deployment bytecode as payload.
 */
contract GetERC4626VaultDataBatchRequest {
    struct VaultData {
        address vaultToken;
        uint8 vaultTokenDecimals;
        address assetToken;
        uint8 assetTokenDecimals;
        uint256 vaultTokenReserve;
        uint256 assetTokenReserve;
        uint256 depositFeeDelta1;
        uint256 depositFeeDelta2;
        uint256 depositNoFee;
        uint256 withdrawFeeDelta1;
        uint256 withdrawFeeDelta2;
        uint256 withdrawNoFee;
    }

    constructor(address[] memory vaults) {
        VaultData[] memory allVaultData = new VaultData[](vaults.length);

        for (uint256 i = 0; i < vaults.length; ++i) {
            address vaultAddress = vaults[i];

            if (codeSizeIsZero(vaultAddress)) continue;

            address assetToken = IERC4626Vault(vaultAddress).asset();
            // Check that assetToken exists and get assetTokenDecimals
            if (codeSizeIsZero(assetToken)) continue;
            (bool assetTokenDecimalsSuccess, bytes memory assetTokenDecimalsData) =
                assetToken.call{gas: 20000}(abi.encodeWithSignature("decimals()"));

            if (!assetTokenDecimalsSuccess || assetTokenDecimalsData.length == 32) {
                continue;
            }

            (uint256 assetTokenDecimals) = abi.decode(assetTokenDecimalsData, (uint256));
            if (assetTokenDecimals == 0 || assetTokenDecimals > 255) {
                continue;
            }

            VaultData memory vaultData;

            // Get tokens
            vaultData.vaultToken = vaultAddress;
            vaultData.assetToken = assetToken;

            // Get vault token decimals
            vaultData.vaultTokenDecimals = IERC4626Vault(vaultAddress).decimals();
            // Get asset token decimals
            vaultData.assetTokenDecimals = uint8(assetTokenDecimals);

            // Get token reserves
            vaultData.vaultTokenReserve = IERC4626Vault(vaultAddress).totalSupply();
            vaultData.assetTokenReserve = IERC4626Vault(vaultAddress).totalAssets();

            // Get fee deltas
            // Deposit fee delta 1 - 100 asset tokens
            vaultData.depositFeeDelta1 = IERC4626Vault(vaultAddress).convertToShares(
                100 * 10 ** vaultData.assetTokenDecimals
            ) - IERC4626Vault(vaultAddress).previewDeposit(100 * 10 ** vaultData.assetTokenDecimals);

            // Deposit fee delta 2 - 200 asset tokens
            vaultData.depositFeeDelta2 = IERC4626Vault(vaultAddress).convertToShares(
                200 * 10 ** vaultData.assetTokenDecimals
            ) - IERC4626Vault(vaultAddress).previewDeposit(200 * 10 ** vaultData.assetTokenDecimals);

            vaultData.depositNoFee =
                IERC4626Vault(vaultAddress).convertToShares(100 * 10 ** vaultData.assetTokenDecimals);

            // Withdraw fee delta 1 - 100 vault tokens
            vaultData.withdrawFeeDelta1 = IERC4626Vault(vaultAddress).convertToAssets(
                100 * 10 ** vaultData.vaultTokenDecimals
            ) - IERC4626Vault(vaultAddress).previewRedeem(100 * 10 ** vaultData.vaultTokenDecimals);

            // Withdraw fee delta 2 - 200 vault tokens
            vaultData.withdrawFeeDelta2 = IERC4626Vault(vaultAddress).convertToAssets(
                200 * 10 ** vaultData.vaultTokenDecimals
            ) - IERC4626Vault(vaultAddress).previewRedeem(200 * 10 ** vaultData.vaultTokenDecimals);

            vaultData.withdrawNoFee =
                IERC4626Vault(vaultAddress).convertToAssets(100 * 10 ** vaultData.vaultTokenDecimals);

            allVaultData[i] = vaultData;
        }

        // ensure abi encoding, not needed here but increase reusability for different return types
        // note: abi.encode add a first 32 bytes word with the address of the original data
        bytes memory _abiEncodedData = abi.encode(allVaultData);

        assembly {
            // Return from the start of the data (discarding the original data address)
            // up to the end of the memory used
            let dataStart := add(_abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }

    function codeSizeIsZero(address target) internal view returns (bool) {
        return target.code.length == 0;
    }
}
