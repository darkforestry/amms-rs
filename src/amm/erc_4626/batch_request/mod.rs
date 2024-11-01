use std::sync::Arc;

use crate::{amm::AutomatedMarketMaker, errors::AMMError};

use alloy::{
    network::Network,
    primitives::{Address, U256},
    providers::Provider,
    sol,
    sol_types::SolValue,
    transports::Transport,
};

use super::ERC4626Vault;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetERC4626VaultDataBatchRequest,
    "src/amm/erc_4626/batch_request/GetERC4626VaultDataBatchRequestABI.json"
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
        IGetERC4626VaultDataBatchRequest::deploy_builder(provider, vec![vault.vault_token]);
    let res = deployer.call_raw().await?;

    let data = <Vec<(
        Address,
        u16,
        Address,
        u16,
        U256,
        U256,
        U256,
        U256,
        U256,
        U256,
        U256,
        U256,
    )> as SolValue>::abi_decode(&res, false)?;
    let (
        vault_token,
        vault_token_dec,
        asset_token,
        asset_token_dec,
        vault_reserve,
        asset_reserve,
        deposit_fee_delta_1,
        deposit_fee_delta_2,
        deposit_no_fee,
        withdraw_fee_delta_1,
        withdraw_fee_delta_2,
        withdraw_no_fee,
    ) = if !data.is_empty() {
        data[0]
    } else {
        return Err(AMMError::BatchRequestError(vault.address()));
    };

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
        vault.withdraw_fee = (withdraw_fee_delta_1 / (withdraw_no_fee / U256::from(10_000))).to();
    } else {
        // If not a relative fee or zero, ignore vault
        return Err(AMMError::BatchRequestError(vault.address()));
    }

    // if above does not error => populate the vault
    vault.vault_token = vault_token;
    vault.vault_token_decimals = vault_token_dec as u8;
    vault.asset_token = asset_token;
    vault.asset_token_decimals = asset_token_dec as u8;
    vault.vault_reserve = vault_reserve;
    vault.asset_reserve = asset_reserve;

    Ok(())
}
