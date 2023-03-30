use ethers::types::H160;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ERC4626 {
    pub address: H160,
    pub vault_token: H160, // token received from depositing, i.e. shares token
    pub vault_token_decimals: u8,
    pub asset_token: H160, // token received from withdrawing, i.e. underlying token
    pub asset_token_decimals: u8,
    pub vault_reserve: u128, // total supply of vault tokens
    pub asset_reserve: u128, // total balance of asset tokens held by vault
    pub fee: u32,
}
