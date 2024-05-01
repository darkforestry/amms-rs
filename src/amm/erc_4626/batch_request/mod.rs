use std::sync::Arc;

use crate::{amm::AutomatedMarketMaker, errors::AMMError};

use alloy::{network::Network, primitives::U256, providers::Provider, sol, transports::Transport};

use super::ERC4626Vault;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetERC4626VaultDataBatchRequest,
    "src/amm/erc_4626/batch_request/GetERC4626VaultDataBatchRequestABI.json"
}

sol! {
    contract IGetERC4626VaultDataBatchReturn {
        function constructorReturn() external view returns ((address, uint8, address, uint8, uint256, uint256, uint256, uint256, uint256, uint256, uint256, uint256)[] memory);
    }
}

pub async fn get_4626_vault_data_batch_request<T, N, P>(
    vault: &mut ERC4626Vault,
    provider: Arc<P>,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let deployer =
        IGetERC4626VaultDataBatchRequest::deploy_builder(provider.clone(), vec![vault.vault_token])
            .with_sol_decoder::<IGetERC4626VaultDataBatchReturn::constructorReturnCall>();
    let IGetERC4626VaultDataBatchReturn::constructorReturnReturn { _0: vault_data } =
        deployer.call().await?;

    // make sure returned vault data len == 1
    let vault_data_len = vault_data.len();
    if vault_data_len != 1_usize {
        return Err(AMMError::EyreError(eyre::eyre!(
            "Unexpected return length, expected 1, returned {vault_data_len}"
        )));
    }

    if !vault_data[0].0.is_zero() {
        let deposit_fee_delta_1 = vault_data[0].6;
        let deposit_fee_delta_2 = vault_data[0].7;
        let deposit_no_fee = vault_data[0].8;
        let withdraw_fee_delta_1 = vault_data[0].9;
        let withdraw_fee_delta_2 = vault_data[0].10;
        let withdraw_no_fee = vault_data[0].11;

        // If both deltas are zero, the fee is zero
        if deposit_fee_delta_1.is_zero() && deposit_fee_delta_2.is_zero() {
            vault.deposit_fee = 0;
        // Assuming 18 decimals, if the delta of 1e20 is half the delta of 2e20, relative fee.
        // Delta / (amount without fee / 10000) to give us the fee in basis points
        } else if deposit_fee_delta_1 * U256::from(2) == deposit_fee_delta_2 {
            vault.deposit_fee = (deposit_fee_delta_1 / (deposit_no_fee / U256::from(10_000))).to();
        } else {
            // If not a relative fee or zero, ignore vault
            return Err(AMMError::BatchRequestError(vault.address()));
        }

        // If both deltas are zero, the fee is zero
        if withdraw_fee_delta_1.is_zero() && withdraw_fee_delta_2.is_zero() {
            vault.withdraw_fee = 0;
        // Assuming 18 decimals, if the delta of 1e20 is half the delta of 2e20, relative fee.
        // Delta / (amount without fee / 10000) to give us the fee in basis points
        } else if withdraw_fee_delta_1 * U256::from(2) == withdraw_fee_delta_2 {
            vault.withdraw_fee =
                (withdraw_fee_delta_1 / (withdraw_no_fee / U256::from(10_000))).to();
        } else {
            // If not a relative fee or zero, ignore vault
            return Err(AMMError::BatchRequestError(vault.address()));
        }

        // if above does not error => populate the vault
        vault.vault_token = vault_data[0].0;
        vault.vault_token_decimals = vault_data[0].1;
        vault.asset_token = vault_data[0].2;
        vault.asset_token_decimals = vault_data[0].3;
        vault.vault_reserve = vault_data[0].4;
        vault.asset_reserve = vault_data[0].5;
    }

    Ok(())
}
