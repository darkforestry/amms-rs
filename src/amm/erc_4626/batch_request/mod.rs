use ethers::{
    abi::{ParamType, Token},
    providers::Middleware,
    types::{Bytes, U256},
};
use std::sync::Arc;

use crate::errors::DAMMError;

use ethers::prelude::abigen;

use super::ERC4626Vault;

abigen!(
    IGetERC4626VaultDataBatchRequest,
        "src/amm/erc_4626/batch_request/GetERC4626VaultDataBatchRequestABI.json";
);

pub async fn get_4626_vault_data_batch_request<M: Middleware>(
    vault: &mut ERC4626Vault,
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    let constructor_args =
        Token::Tuple(vec![Token::Array(vec![Token::Address(vault.vault_token)])]);

    let deployer =
        IGetERC4626VaultDataBatchRequest::deploy(middleware.clone(), constructor_args).unwrap();

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
            ParamType::Uint(256), // withdraw fee delta 1
            ParamType::Uint(256), // withdraw fee delta 2
        ])))],
        &return_data,
    )?;

    for tokens in return_data_tokens {
        if let Some(tokens_arr) = tokens.into_array() {
            for tup in tokens_arr {
                if let Some(vault_data) = tup.into_tuple() {
                    // If the vault token is not zero, signalling that the vault data was populated
                    if !vault_data[0].to_owned().into_address().unwrap().is_zero() {
                        vault.vault_token = vault_data[0].to_owned().into_address().unwrap();
                        vault.vault_token_decimals =
                            vault_data[1].to_owned().into_uint().unwrap().as_u32() as u8;
                        vault.asset_token = vault_data[2].to_owned().into_address().unwrap();
                        vault.asset_token_decimals =
                            vault_data[3].to_owned().into_uint().unwrap().as_u32() as u8;
                        vault.vault_reserve = vault_data[4].to_owned().into_uint().unwrap();
                        vault.asset_reserve = vault_data[5].to_owned().into_uint().unwrap();

                        let deposit_fee_delta_1 = vault_data[6].to_owned().into_uint().unwrap();
                        let deposit_fee_delta_2 = vault_data[7].to_owned().into_uint().unwrap();
                        let withdraw_fee_delta_1 = vault_data[8].to_owned().into_uint().unwrap();
                        let withdraw_fee_delta_2 = vault_data[9].to_owned().into_uint().unwrap();

                        // If both deltas are zero, the fee is zero
                        if deposit_fee_delta_1.is_zero() && deposit_fee_delta_2.is_zero() {
                            vault.deposit_fee = 0;
                        // Assuming 18 decimals, if the delta of 1e20 is half the delta of 2e20, relative fee.
                        // Delta from 1e20 divided by 1e16 to give us the fee in basis points
                        } else if deposit_fee_delta_1 * 2 == deposit_fee_delta_2 {
                            vault.deposit_fee = (deposit_fee_delta_1
                                / U256::from(10u128.pow(vault.vault_token_decimals.into())))
                            .as_u32();
                        } else {
                            // If not a relative fee or zero, ignore vault
                            return Err(DAMMError::InvalidERC4626Fee);
                        }

                        // Assuming 18 decimals, if both deltas are zero, the fee is zero
                        if withdraw_fee_delta_1.is_zero() && withdraw_fee_delta_2.is_zero() {
                            vault.withdraw_fee = 0;
                        // If the delta of 1e20 is half the delta of 2e20, relative fee.
                        // Delta from 1e20 divided by 1e16 to give us the fee in basis points
                        } else if withdraw_fee_delta_1 * 2 == withdraw_fee_delta_2 {
                            vault.withdraw_fee = (withdraw_fee_delta_1
                                / U256::from(10u128.pow(vault.asset_token_decimals.into())))
                            .as_u32();
                        } else {
                            // If not a relative fee or zero, ignore vault
                            return Err(DAMMError::InvalidERC4626Fee);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
