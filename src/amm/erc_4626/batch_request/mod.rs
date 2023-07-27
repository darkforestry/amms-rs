use ethers::{
    abi::{ParamType, Token},
    providers::Middleware,
    types::{Bytes, U256},
};
use std::sync::Arc;

use crate::{amm::AutomatedMarketMaker, errors::AMMError};

use ethers::prelude::abigen;

use super::ERC4626Vault;

abigen!(
    IGetERC4626VaultDataBatchRequest,
        "src/amm/erc_4626/batch_request/GetERC4626VaultDataBatchRequestABI.json";
);

fn populate_vault_data_from_tokens(
    mut vault: ERC4626Vault,
    tokens: Vec<Token>,
) -> Option<ERC4626Vault> {
    vault.vault_token = tokens[0].to_owned().into_address()?;
    vault.vault_token_decimals = tokens[1].to_owned().into_uint()?.as_u32() as u8;
    vault.asset_token = tokens[2].to_owned().into_address()?;
    vault.asset_token_decimals = tokens[3].to_owned().into_uint()?.as_u32() as u8;
    vault.vault_reserve = tokens[4].to_owned().into_uint()?;
    vault.asset_reserve = tokens[5].to_owned().into_uint()?;

    let deposit_fee_delta_1 = tokens[6].to_owned().into_uint()?;
    let deposit_fee_delta_2 = tokens[7].to_owned().into_uint()?;
    let deposit_no_fee = tokens[8].to_owned().into_uint()?;
    let withdraw_fee_delta_1 = tokens[9].to_owned().into_uint()?;
    let withdraw_fee_delta_2 = tokens[10].to_owned().into_uint()?;
    let withdraw_no_fee = tokens[11].to_owned().into_uint()?;

    // If both deltas are zero, the fee is zero
    if deposit_fee_delta_1.is_zero() && deposit_fee_delta_2.is_zero() {
        vault.deposit_fee = 0;
    // Assuming 18 decimals, if the delta of 1e20 is half the delta of 2e20, relative fee.
    // Delta / (amount without fee / 10000) to give us the fee in basis points
    } else if deposit_fee_delta_1 * 2 == deposit_fee_delta_2 {
        vault.deposit_fee =
            (deposit_fee_delta_1 / (deposit_no_fee / U256::from("0x2710"))).as_u32();
    } else {
        // If not a relative fee or zero, ignore vault
        return None;
    }

    // If both deltas are zero, the fee is zero
    if withdraw_fee_delta_1.is_zero() && withdraw_fee_delta_2.is_zero() {
        vault.withdraw_fee = 0;
    // Assuming 18 decimals, if the delta of 1e20 is half the delta of 2e20, relative fee.
    // Delta / (amount without fee / 10000) to give us the fee in basis points
    } else if withdraw_fee_delta_1 * 2 == withdraw_fee_delta_2 {
        vault.withdraw_fee =
            (withdraw_fee_delta_1 / (withdraw_no_fee / U256::from("0x2710"))).as_u32();
    } else {
        // If not a relative fee or zero, ignore vault
        return None;
    }

    Some(vault)
}

pub async fn get_4626_vault_data_batch_request<M: Middleware>(
    vault: &mut ERC4626Vault,
    middleware: Arc<M>,
) -> Result<(), AMMError<M>> {
    let constructor_args =
        Token::Tuple(vec![Token::Array(vec![Token::Address(vault.vault_token)])]);

    let deployer = IGetERC4626VaultDataBatchRequest::deploy(middleware.clone(), constructor_args)?;

    let return_data: Bytes = deployer.call_raw().await?;
    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Tuple(vec![
            ParamType::Address,   // vault token
            ParamType::Uint(8),   // vault token decimals
            ParamType::Address,   // asset token
            ParamType::Uint(8),   // asset token decimals
            ParamType::Uint(256), // vault token reserve
            ParamType::Uint(256), // asset token reserve
            ParamType::Uint(256), // deposit fee delta 1
            ParamType::Uint(256), // deposit fee delta 2
            ParamType::Uint(256), // deposit not fee
            ParamType::Uint(256), // withdraw fee delta 1
            ParamType::Uint(256), // withdraw fee delta 2
            ParamType::Uint(256), // withdraw no fee
        ])))],
        &return_data,
    )?;

    for tokens in return_data_tokens {
        if let Some(tokens_arr) = tokens.into_array() {
            for tup in tokens_arr {
                let vault_data = tup
                    .into_tuple()
                    .ok_or(AMMError::BatchRequestError(vault.address()))?;

                *vault = populate_vault_data_from_tokens(vault.to_owned(), vault_data)
                    .ok_or(AMMError::BatchRequestError(vault.address()))?;
            }
        }
    }

    Ok(())
}
