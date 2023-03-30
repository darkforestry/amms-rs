use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{H160, H256},
};
use serde::{Deserialize, Serialize};

use crate::{
    amm::AutomatedMarketMaker,
    errors::{ArithmeticError, DAMMError},
};

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
    pub address: H160,
    pub vault_token: H160, // token received from depositing, i.e. shares token
    pub vault_token_decimals: u8,
    pub asset_token: H160, // token received from withdrawing, i.e. underlying token
    pub asset_token_decimals: u8,
    pub vault_reserve: u128, // total supply of vault tokens
    pub asset_reserve: u128, // total balance of asset tokens held by vault
    pub fee: u32,
}

#[async_trait]
impl AutomatedMarketMaker for ERC4626Vault {
    fn address(&self) -> H160 {
        self.address
    }

    fn tokens(&self) -> Vec<H160> {
        vec![self.vault_token, self.asset_token]
    }

    // TODO: implement
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        Ok(0.0)
    }

    // TODO: implement
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
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
        address: H160,
        vault_token: H160,
        vault_token_decimals: u8,
        asset_token: H160,
        asset_token_decimals: u8,
        vault_reserve: u128,
        asset_reserve: u128,
        fee: u32,
    ) -> ERC4626Vault {
        ERC4626Vault {
            address,
            vault_token,
            vault_token_decimals,
            asset_token,
            asset_token_decimals,
            vault_reserve,
            asset_reserve,
            fee,
        }
    }
}
