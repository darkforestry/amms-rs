use ethers::{
    abi::{ParamType, Token},
    providers::Middleware,
    types::Bytes,
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
                            vault_data[1].to_owned().into_uint().unwrap().as_u8();
                        vault.asset_token = vault_data[2].to_owned().into_address().unwrap();
                        vault.asset_token_decimals =
                            vault_data[3].to_owned().into_uint().unwrap().as_u8();
                        vault.vault_reserve = vault_data[4].to_owned().into_uint().unwrap();
                        vault.asset_reserve = vault_data[5].to_owned().into_uint().unwrap();
                        // TODO: Add fee
                        vault.fee = 0;
                    }
                }
            }
        }
    }

    Ok(())
}
