pub mod batch_request;

use std::{cmp::Ordering, sync::Arc};

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

use super::uniswap_v2::{div_uu, q64_to_f64};

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

    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        Ok(q64_to_f64(self.calculate_price_64_x_64(base_token)?))
    }

    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
        (self.vault_reserve, self.asset_reserve) = self.get_reserves(middleware).await?;

        Ok(())
    }

    fn sync_on_event_signatures(&self) -> Vec<H256> {
        vec![DEPOSIT_EVENT_SIGNATURE, WITHDRAW_EVENT_SIGNATURE]
    }

    async fn populate_data<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        batch_request::get_4626_vault_data_batch_request(self, middleware.clone()).await?;

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

    // TODO: Include fee
    pub fn calculate_price_64_x_64(&self, base_token: H160) -> Result<u128, ArithmeticError> {
        let decimal_shift = self.vault_token_decimals as i8 - self.asset_token_decimals as i8;

        let (r_v, r_a) = match decimal_shift.cmp(&0) {
            Ordering::Less => (
                U256::from(self.vault_reserve)
                    * U256::from(10u128.pow(decimal_shift.unsigned_abs() as u32)),
                U256::from(self.asset_reserve),
            ),
            _ => (
                U256::from(self.vault_reserve),
                U256::from(self.asset_reserve) * U256::from(10u128.pow(decimal_shift as u32)),
            ),
        };

        if base_token == self.vault_token {
            if r_v == U256::zero() {
                return Ok(10u128.pow(self.vault_token_decimals as u32));
            } else {
                Ok(div_uu(r_a, r_v)?)
            }
        } else {
            if r_a == U256::zero() {
                return Ok(10u128.pow(self.asset_token_decimals as u32));
            } else {
                Ok(div_uu(r_v, r_a)?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use ethers::{
        providers::{Http, Provider},
        types::{H160, U256},
    };

    use crate::amm::AutomatedMarketMaker;

    use super::ERC4626Vault;

    #[tokio::test]
    async fn test_get_vault_data() {
        let rpc_endpoint =
            std::env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let mut vault = ERC4626Vault {
            vault_token: H160::from_str("0x163538E22F4d38c1eb21B79939f3d2ee274198Ff").unwrap(),
            ..Default::default()
        };

        vault.populate_data(middleware).await.unwrap();

        assert_eq!(vault.vault_token_decimals, 18);
        assert_eq!(
            vault.asset_token,
            H160::from_str("0x6B175474E89094C44Da98b954EedeAC495271d0F").unwrap()
        );
        assert_eq!(vault.asset_token_decimals, 18);
        assert_eq!(vault.fee, 0);
    }

    #[tokio::test]
    async fn test_calculate_price() {
        let rpc_endpoint =
            std::env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let mut vault = ERC4626Vault {
            vault_token: H160::from_str("0x163538E22F4d38c1eb21B79939f3d2ee274198Ff").unwrap(),
            ..Default::default()
        };

        vault.populate_data(middleware).await.unwrap();

        vault.vault_reserve = U256::from_dec_str("501910315708981197269904").unwrap();
        vault.asset_reserve = U256::from_dec_str("505434849031054568651911").unwrap();

        let price_v_64_x = vault.calculate_price(vault.vault_token).unwrap();
        let price_a_64_x = vault.calculate_price(vault.asset_token).unwrap();

        assert_eq!(price_v_64_x, 1.0070222372638322);
        assert_eq!(price_a_64_x, 0.9930267306877828);
    }

    #[tokio::test]
    async fn test_calculate_price_64_x_64() {
        let rpc_endpoint =
            std::env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

        let mut vault = ERC4626Vault {
            vault_token: H160::from_str("0x163538E22F4d38c1eb21B79939f3d2ee274198Ff").unwrap(),
            ..Default::default()
        };

        vault.populate_data(middleware).await.unwrap();

        vault.vault_reserve = U256::from_dec_str("501910315708981197269904").unwrap();
        vault.asset_reserve = U256::from_dec_str("505434849031054568651911").unwrap();

        let price_v_64_x = vault.calculate_price_64_x_64(vault.vault_token).unwrap();
        let price_a_64_x = vault.calculate_price_64_x_64(vault.asset_token).unwrap();

        assert_eq!(price_v_64_x, 18576281487340329878);
        assert_eq!(price_a_64_x, 18318109959350028841);
    }
}
