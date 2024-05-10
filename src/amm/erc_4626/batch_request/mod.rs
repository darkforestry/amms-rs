use std::sync::Arc;

use crate::{amm::AutomatedMarketMaker, errors::AMMError};

use alloy::{
    dyn_abi::{DynSolType, DynSolValue},
    network::Network,
    primitives::U256,
    providers::Provider,
    sol,
    transports::Transport,
};

use super::ERC4626Vault;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetERC4626VaultDataBatchRequest,
    "src/amm/erc_4626/batch_request/GetERC4626VaultDataBatchRequestABI.json"
}

#[inline]
fn populate_pool_data_from_tokens(
    mut vault: ERC4626Vault,
    tokens: &[DynSolValue],
) -> Option<ERC4626Vault> {
    let deposit_fee_delta_1 = tokens[6].as_uint()?.0;
    let deposit_fee_delta_2 = tokens[7].as_uint()?.0;
    let deposit_no_fee = tokens[8].as_uint()?.0;
    let withdraw_fee_delta_1 = tokens[9].as_uint()?.0;
    let withdraw_fee_delta_2 = tokens[10].as_uint()?.0;
    let withdraw_no_fee = tokens[11].as_uint()?.0;

    // If both deltas are zero, the fee is zero
    if deposit_fee_delta_1.is_zero() && deposit_fee_delta_2.is_zero() {
        vault.deposit_fee = 0;
    // Assuming 18 decimals, if the delta of 1e20 is half the delta of 2e20, relative fee.
    // Delta / (amount without fee / 10000) to give us the fee in basis points
    } else if deposit_fee_delta_1 * U256::from(2) == deposit_fee_delta_2 {
        vault.deposit_fee = (deposit_fee_delta_1 / (deposit_no_fee / U256::from(10_000))).to();
    } else {
        // If not a relative fee or zero, ignore vault
        return None;
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
        return None;
    }

    // if above does not error => populate the vault
    vault.vault_token = tokens[0].as_address()?;
    vault.vault_token_decimals = tokens[1].as_uint()?.0.to::<u8>();
    vault.asset_token = tokens[2].as_address()?;
    vault.asset_token_decimals = tokens[3].as_uint()?.0.to::<u8>();
    vault.vault_reserve = tokens[4].as_uint()?.0;
    vault.asset_reserve = tokens[5].as_uint()?.0;

    Some(vault)
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

    let constructor_return = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
        DynSolType::Address,
        DynSolType::Uint(8),
        DynSolType::Address,
        DynSolType::Uint(8),
        DynSolType::Uint(256),
        DynSolType::Uint(256),
        DynSolType::Uint(256),
        DynSolType::Uint(256),
        DynSolType::Uint(256),
        DynSolType::Uint(256),
        DynSolType::Uint(256),
        DynSolType::Uint(256),
    ])));
    let return_data_tokens = constructor_return.abi_decode_sequence(&res)?;

    if let Some(tokens_arr) = return_data_tokens.as_array() {
        for token in tokens_arr {
            let vault_data = token
                .as_tuple()
                .ok_or(AMMError::BatchRequestError(vault.address()))?;

            *vault = populate_pool_data_from_tokens(vault.to_owned(), vault_data)
                .ok_or(AMMError::BatchRequestError(vault.address()))?;
        }
    }

    Ok(())
}
