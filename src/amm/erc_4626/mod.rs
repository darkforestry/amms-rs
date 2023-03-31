use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{H160, H256, U256},
};
use serde::{Deserialize, Serialize};

use crate::{
    amm::AutomatedMarketMaker,
    errors::{ArithmeticError, DAMMError},
};

use ethers::prelude::abigen;

abigen!(
    IERC4626Vault,
    r#"[
        function totalAssets() external view returns (uint256)
        function totalSupply() external view returns (uint256)
        function decimals() external view returns (uint8)
    ]"#;
);

pub const DEPOSIT_EVENT_SIGNATURE: H256 = H256([
    220, 188, 28, 5, 36, 15, 49, 255, 58, 208, 103, 239, 30, 227, 92, 228, 153, 119, 98, 117, 46,
    58, 9, 82, 132, 117, 69, 68, 244, 199, 9, 215,
]);
pub const WITHDRAW_EVENT_SIGNATURE: H256 = H256([
    251, 222, 121, 125, 32, 28, 104, 27, 145, 5, 101, 41, 25, 224, 176, 36, 7, 199, 187, 150, 164,
    162, 199, 92, 1, 252, 150, 103, 114, 50, 200, 219,
]);

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ERC4626Vault {
    pub vault_token: H160, // token received from depositing, i.e. shares token
    pub vault_token_decimals: u8,
    pub asset_token: H160, // token received from withdrawing, i.e. underlying token
    pub asset_token_decimals: u8,
    pub vault_reserve: U256, // total supply of vault tokens
    pub asset_reserve: U256, // total balance of asset tokens held by vault
    pub fee: u32,
}

#[async_trait]
impl AutomatedMarketMaker for ERC4626Vault {
    fn address(&self) -> H160 {
        self.vault_token
    }

    fn tokens(&self) -> Vec<H160> {
        vec![self.vault_token, self.asset_token]
    }

    // TODO: implement
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        Ok(0.0)
    }

    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
        (self.vault_reserve, self.asset_reserve) = self.get_reserves(middleware).await?;

        Ok(())
    }

    fn sync_on_event_signatures(&self) -> Vec<H256> {
        vec![DEPOSIT_EVENT_SIGNATURE, WITHDRAW_EVENT_SIGNATURE]
    }

    // TODO: implement
    async fn populate_data<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        Ok(())
    }
}

impl ERC4626Vault {
    pub fn new(
        vault_token: H160,
        vault_token_decimals: u8,
        asset_token: H160,
        asset_token_decimals: u8,
        vault_reserve: U256,
        asset_reserve: U256,
        fee: u32,
    ) -> ERC4626Vault {
        ERC4626Vault {
            vault_token,
            vault_token_decimals,
            asset_token,
            asset_token_decimals,
            vault_reserve,
            asset_reserve,
            fee,
        }
    }

    pub async fn get_reserves<M: Middleware>(
        &self,
        middleware: Arc<M>,
    ) -> Result<(U256, U256), DAMMError<M>> {
        //Initialize a new instance of the vault
        let vault = IERC4626Vault::new(self.vault_token, middleware);
        // Get the total assets in the vault
        let total_assets = match vault.total_assets().call().await {
            Ok(total_assets) => total_assets,
            Err(e) => return Err(DAMMError::ContractError(e)),
        };
        // Get the total supply of the vault token
        let total_supply = match vault.total_supply().call().await {
            Ok(total_supply) => total_supply,
            Err(e) => return Err(DAMMError::ContractError(e)),
        };

        Ok((total_supply, total_assets))
    }
}
