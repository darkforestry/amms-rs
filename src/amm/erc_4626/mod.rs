pub mod batch_request;

use std::{cmp::Ordering, sync::Arc};

use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::eth::Log,
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    amm::{consts::U128_0X10000000000000000, AutomatedMarketMaker},
    errors::{AMMError, ArithmeticError, EventLogError, SwapSimulationError},
};

use super::uniswap_v2::{div_uu, q64_to_f64};

sol! {
    /// Interface of the IERC4626Valut contract
    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract IERC4626Vault {
        event Withdraw(address indexed sender, address indexed receiver, address indexed owner, uint256 assets, uint256 shares);
        event Deposit(address indexed sender,address indexed owner, uint256 assets, uint256 shares);
        function totalAssets() external view returns (uint256);
        function totalSupply() external view returns (uint256);
        function decimals() external view returns (uint8);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ERC4626Vault {
    /// token received from depositing, i.e. shares token
    pub vault_token: Address,
    pub vault_token_decimals: u8,
    /// token received from withdrawing, i.e. underlying token
    pub asset_token: Address,
    pub asset_token_decimals: u8,
    /// total supply of vault tokens
    pub vault_reserve: U256,
    /// total balance of asset tokens held by vault
    pub asset_reserve: U256,
    /// deposit fee in basis points
    pub deposit_fee: u32,
    /// withdrawal fee in basis points
    pub withdraw_fee: u32,
}

#[async_trait]
impl AutomatedMarketMaker for ERC4626Vault {
    fn address(&self) -> Address {
        self.vault_token
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.vault_token, self.asset_token]
    }

    fn calculate_price(&self, base_token: Address) -> Result<f64, ArithmeticError> {
        Ok(q64_to_f64(self.calculate_price_64_x_64(base_token)?))
    }

    #[instrument(skip(self, provider), level = "debug")]
    async fn sync<T, N, P>(&mut self, provider: Arc<P>) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let (vault_reserve, asset_reserve) = self.get_reserves(provider).await?;
        tracing::debug!(vault_reserve = ?vault_reserve, asset_reserve = ?asset_reserve, address = ?self.vault_token, "ER4626 sync");

        self.vault_reserve = vault_reserve;
        self.asset_reserve = asset_reserve;

        Ok(())
    }

    fn sync_on_event_signatures(&self) -> Vec<B256> {
        vec![
            IERC4626Vault::Deposit::SIGNATURE_HASH,
            IERC4626Vault::Withdraw::SIGNATURE_HASH,
        ]
    }

    #[instrument(skip(self), level = "debug")]
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
        let event_signature = log.data().topics()[0];
        if event_signature == IERC4626Vault::Deposit::SIGNATURE_HASH {
            let deposit_event = IERC4626Vault::Deposit::decode_log(log.as_ref(), true)?;
            self.asset_reserve += deposit_event.assets;
            self.vault_reserve += deposit_event.shares;
            tracing::debug!(asset_reserve = ?self.asset_reserve, vault_reserve = ?self.vault_reserve, address = ?self.vault_token, "ER4626 deposit event");
        } else if event_signature == IERC4626Vault::Withdraw::SIGNATURE_HASH {
            let withdraw_filter = IERC4626Vault::Withdraw::decode_log(log.as_ref(), true)?;
            self.asset_reserve -= withdraw_filter.assets;
            self.vault_reserve -= withdraw_filter.shares;
            tracing::debug!(asset_reserve = ?self.asset_reserve, vault_reserve = ?self.vault_reserve, address = ?self.vault_token, "ER4626 withdraw event");
        } else {
            return Err(EventLogError::InvalidEventSignature);
        }

        Ok(())
    }

    #[instrument(skip(self, provider), level = "debug")]
    async fn populate_data<T, N, P>(
        &mut self,
        _block_number: Option<u64>,
        provider: Arc<P>,
    ) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        batch_request::get_4626_vault_data_batch_request(self, provider.clone()).await?;

        Ok(())
    }

    fn simulate_swap(
        &self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        if self.vault_token == token_in {
            Ok(self.get_amount_out(amount_in, self.vault_reserve, self.asset_reserve))
        } else {
            Ok(self.get_amount_out(amount_in, self.asset_reserve, self.vault_reserve))
        }
    }

    fn simulate_swap_mut(
        &mut self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        if self.vault_token == token_in {
            let amount_out = self.get_amount_out(amount_in, self.vault_reserve, self.asset_reserve);

            self.vault_reserve -= amount_in;
            self.asset_reserve -= amount_out;

            Ok(amount_out)
        } else {
            let amount_out = self.get_amount_out(amount_in, self.asset_reserve, self.vault_reserve);

            self.asset_reserve += amount_in;
            self.vault_reserve += amount_out;

            Ok(amount_out)
        }
    }

    fn get_token_out(&self, token_in: Address) -> Address {
        if self.vault_token == token_in {
            self.asset_token
        } else {
            self.vault_token
        }
    }
}

impl ERC4626Vault {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vault_token: Address,
        vault_token_decimals: u8,
        asset_token: Address,
        asset_token_decimals: u8,
        vault_reserve: U256,
        asset_reserve: U256,
        deposit_fee: u32,
        withdraw_fee: u32,
    ) -> ERC4626Vault {
        ERC4626Vault {
            vault_token,
            vault_token_decimals,
            asset_token,
            asset_token_decimals,
            vault_reserve,
            asset_reserve,
            deposit_fee,
            withdraw_fee,
        }
    }

    pub async fn new_from_address<T, N, P>(
        vault_token: Address,
        provider: Arc<P>,
    ) -> Result<Self, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let mut vault = ERC4626Vault {
            vault_token,
            vault_token_decimals: 0,
            asset_token: Address::ZERO,
            asset_token_decimals: 0,
            vault_reserve: U256::ZERO,
            asset_reserve: U256::ZERO,
            deposit_fee: 0,
            withdraw_fee: 0,
        };

        vault.populate_data(None, provider.clone()).await?;

        if !vault.data_is_populated() {
            return Err(AMMError::PoolDataError);
        }

        Ok(vault)
    }

    pub fn data_is_populated(&self) -> bool {
        !(self.vault_token.is_zero()
            || self.asset_token.is_zero()
            || self.vault_reserve.is_zero()
            || self.asset_reserve.is_zero())
    }

    pub async fn get_reserves<T, N, P>(&self, provider: Arc<P>) -> Result<(U256, U256), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        // Initialize a new instance of the vault
        let vault = IERC4626Vault::new(self.vault_token, provider);

        // Get the total assets in the vault
        let IERC4626Vault::totalAssetsReturn { _0: total_assets } =
            match vault.totalAssets().call().await {
                Ok(total_assets) => total_assets,
                Err(e) => return Err(AMMError::ContractError(e)),
            };

        // Get the total supply of the vault token
        let IERC4626Vault::totalSupplyReturn { _0: total_supply } =
            match vault.totalSupply().call().await {
                Ok(total_supply) => total_supply,
                Err(e) => return Err(AMMError::ContractError(e)),
            };

        Ok((total_supply, total_assets))
    }

    pub fn calculate_price_64_x_64(&self, base_token: Address) -> Result<u128, ArithmeticError> {
        let decimal_shift = self.vault_token_decimals as i8 - self.asset_token_decimals as i8;

        // Normalize reserves by decimal shift
        let (r_v, r_a) = match decimal_shift.cmp(&0) {
            Ordering::Less => (
                self.vault_reserve * U256::from(10u128.pow(decimal_shift.unsigned_abs() as u32)),
                self.asset_reserve,
            ),
            _ => (
                self.vault_reserve,
                self.asset_reserve * U256::from(10u128.pow(decimal_shift as u32)),
            ),
        };

        // Withdraw
        if base_token == self.vault_token {
            if r_v.is_zero() {
                // Return 1 in Q64
                Ok(U128_0X10000000000000000)
            } else {
                Ok(div_uu(r_a, r_v)?)
            }
        // Deposit
        } else if r_a.is_zero() {
            // Return 1 in Q64
            Ok(U128_0X10000000000000000)
        } else {
            Ok(div_uu(r_v, r_a)?)
        }
    }

    pub fn get_amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in.is_zero() {
            return U256::ZERO;
        }

        if self.vault_reserve.is_zero() {
            return amount_in;
        }

        let fee = if reserve_in == self.vault_reserve {
            self.withdraw_fee
        } else {
            self.deposit_fee
        };

        amount_in * reserve_out / reserve_in * U256::from(10000 - fee) / U256::from(10000)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::{
        primitives::{address, U256},
        providers::ProviderBuilder,
    };

    use crate::amm::AutomatedMarketMaker;

    use super::ERC4626Vault;

    #[tokio::test]
    async fn test_get_vault_data() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut vault = ERC4626Vault {
            vault_token: address!("163538E22F4d38c1eb21B79939f3d2ee274198Ff"),
            ..Default::default()
        };

        vault.populate_data(None, provider).await.unwrap();

        assert_eq!(vault.vault_token_decimals, 18);
        assert_eq!(
            vault.asset_token,
            address!("6B175474E89094C44Da98b954EedeAC495271d0F")
        );
        assert_eq!(vault.asset_token_decimals, 18);
        assert_eq!(vault.deposit_fee, 0);
        assert_eq!(vault.withdraw_fee, 0);
    }

    #[tokio::test]
    async fn test_calculate_price_varying_decimals() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut vault = ERC4626Vault {
            vault_token: address!("163538E22F4d38c1eb21B79939f3d2ee274198Ff"),
            ..Default::default()
        };

        vault.populate_data(None, provider).await.unwrap();

        vault.vault_reserve = U256::from(501910315708981197269904_u128);
        vault.asset_token_decimals = 6;
        vault.asset_reserve = U256::from(505434849031_u64);

        let price_v_64_x = vault.calculate_price(vault.vault_token).unwrap();
        let price_a_64_x = vault.calculate_price(vault.asset_token).unwrap();

        assert_eq!(price_v_64_x, 1.0070222372637234);
        assert_eq!(price_a_64_x, 0.99302673068789);
    }

    #[tokio::test]
    async fn test_calculate_price_zero_reserve() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut vault = ERC4626Vault {
            vault_token: address!("163538E22F4d38c1eb21B79939f3d2ee274198Ff"),
            ..Default::default()
        };

        vault.populate_data(None, provider).await.unwrap();

        vault.vault_reserve = U256::ZERO;
        vault.asset_reserve = U256::ZERO;

        let price_v_64_x = vault.calculate_price(vault.vault_token).unwrap();
        let price_a_64_x = vault.calculate_price(vault.asset_token).unwrap();

        assert_eq!(price_v_64_x, 1.0);
        assert_eq!(price_a_64_x, 1.0);
    }

    #[tokio::test]
    async fn test_calculate_price() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut vault = ERC4626Vault {
            vault_token: address!("163538E22F4d38c1eb21B79939f3d2ee274198Ff"),
            ..Default::default()
        };

        vault.populate_data(None, provider).await.unwrap();

        vault.vault_reserve = U256::from(501910315708981197269904_u128);
        vault.asset_reserve = U256::from(505434849031054568651911_u128);

        let price_v_64_x = vault.calculate_price(vault.vault_token).unwrap();
        let price_a_64_x = vault.calculate_price(vault.asset_token).unwrap();

        assert_eq!(price_v_64_x, 1.0070222372638322);
        assert_eq!(price_a_64_x, 0.9930267306877828);
    }

    #[tokio::test]
    async fn test_calculate_price_64_x_64() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut vault = ERC4626Vault {
            vault_token: address!("163538E22F4d38c1eb21B79939f3d2ee274198Ff"),
            ..Default::default()
        };

        vault.populate_data(None, provider).await.unwrap();

        vault.vault_reserve = U256::from(501910315708981197269904_u128);
        vault.asset_reserve = U256::from(505434849031054568651911_u128);

        let price_v_64_x = vault.calculate_price_64_x_64(vault.vault_token).unwrap();
        let price_a_64_x = vault.calculate_price_64_x_64(vault.asset_token).unwrap();

        assert_eq!(price_v_64_x, 18576281487340329878);
        assert_eq!(price_a_64_x, 18318109959350028841);
    }

    #[tokio::test]
    async fn test_simulate_swap() {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
        let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));

        let mut vault = ERC4626Vault {
            vault_token: address!("163538E22F4d38c1eb21B79939f3d2ee274198Ff"),
            ..Default::default()
        };

        vault.populate_data(None, provider).await.unwrap();

        vault.vault_reserve = U256::from(501910315708981197269904_u128);
        vault.asset_reserve = U256::from(505434849031054568651911_u128);

        let assets_out = vault
            .simulate_swap(vault.vault_token, U256::from(3000000000000000000_u128))
            .unwrap();
        let shares_out = vault
            .simulate_swap(vault.asset_token, U256::from(3000000000000000000_u128))
            .unwrap();

        assert_eq!(assets_out, U256::from(3021066711791496478_u128));
        assert_eq!(shares_out, U256::from(2979080192063348487_u128));
    }
}
