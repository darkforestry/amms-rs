use std::sync::Arc;

use async_trait::async_trait;
use ethers::{providers::Middleware, types::H160};
use serde::{Deserialize, Serialize};

use crate::{
    amm::AutomatedMarketMaker,
    errors::{ArithmeticError, DAMMError},
};

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

    // TODO: implement
    fn sync_on_event_signatures(&self) -> Vec<H256> {
        vec![]
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
