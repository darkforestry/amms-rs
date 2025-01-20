use super::{
    amm::AutomatedMarketMaker,
    consts::{F64_FEE_ONE, U256_2, U256_FEE_ONE, U32_FEE_ONE},
    error::AMMError,
    float::u256_to_f64,
    Token,
};
use alloy::{
    eips::BlockId,
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::{SolEvent, SolValue},
    transports::Transport,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

sol! {
    /// Interface of the IERC4626Valut contract
    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract IERC4626Vault {
        event Withdraw(address indexed sender, address indexed receiver, address indexed owner, uint256 assets, uint256 shares);
        event Deposit(address indexed sender,address indexed owner, uint256 assets, uint256 shares);
        function totalAssets() external view returns (uint256);
        function totalSupply() external view returns (uint256);
    }
}

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetERC4626VaultDataBatchRequest,
    "contracts/out/GetERC4626VaultDataBatchRequest.sol/GetERC4626VaultDataBatchRequest.json",
}

#[derive(Error, Debug)]
pub enum ERC4626VaultError {
    #[error("Non relative or zero fee")]
    NonRelativeOrZeroFee,
    #[error("Division by zero")]
    DivisionByZero,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ERC4626Vault {
    /// Token received from depositing, i.e. shares token
    pub vault_token: Token,
    /// Token received from withdrawing, i.e. underlying token
    pub asset_token: Token,
    /// Total supply of vault tokens
    pub vault_reserve: U256,
    /// Total balance of asset tokens held by vault
    pub asset_reserve: U256,
    /// Deposit fee in basis points
    pub deposit_fee: u32,
    /// Withdrawal fee in basis points
    pub withdraw_fee: u32,
}

impl AutomatedMarketMaker for ERC4626Vault {
    fn address(&self) -> Address {
        self.vault_token.address
    }

    fn sync_events(&self) -> Vec<B256> {
        vec![
            IERC4626Vault::Deposit::SIGNATURE_HASH,
            IERC4626Vault::Withdraw::SIGNATURE_HASH,
        ]
    }

    fn sync(&mut self, log: &Log) -> Result<(), AMMError> {
        let event_signature = log.data().topics()[0];
        match event_signature {
            IERC4626Vault::Deposit::SIGNATURE_HASH => {
                let deposit_event = IERC4626Vault::Deposit::decode_log(log.as_ref(), false)?;
                self.asset_reserve += deposit_event.assets;
                self.vault_reserve += deposit_event.shares;

                info!(
                    target = "amms::erc_4626::sync",
                    address = ?self.vault_token,
                    asset_reserve = ?self.asset_reserve,
                    vault_reserve = ?self.vault_reserve,
                    "Deposit"
                );
            }

            IERC4626Vault::Withdraw::SIGNATURE_HASH => {
                let withdraw_event = IERC4626Vault::Withdraw::decode_log(log.as_ref(), false)?;
                self.asset_reserve -= withdraw_event.assets;
                self.vault_reserve -= withdraw_event.shares;

                info!(
                    target = "amms::erc_4626::sync",
                    address = ?self.vault_token,
                    asset_reserve = ?self.asset_reserve,
                    vault_reserve = ?self.vault_reserve,
                    "Withdraw"
                );
            }

            _ => {
                return Err(AMMError::UnrecognizedEventSignature(event_signature));
            }
        }

        Ok(())
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.vault_token.address, self.asset_token.address]
    }

    fn calculate_price(&self, base_token: Address, _quote_token: Address) -> Result<f64, AMMError> {
        // TODO: this is the same behavior as before, but I'm not sure it's correct
        if base_token == self.vault_token {
            if self.vault_reserve == U256::ZERO {
                return Ok(1.0);
            }
        } else {
            if self.asset_reserve == U256::ZERO {
                return Ok(1.0);
            }
        }

        // Decimals are intentionally swapped as we are multiplying rather than dividing
        let (r_a, r_v) = (
            u256_to_f64(self.asset_reserve) * (10f64).powi(self.vault_token.decimals as i32),
            u256_to_f64(self.vault_reserve) * (10f64).powi(self.asset_token.decimals as i32),
        );
        let (reserve_in, reserve_out, fee) = if base_token == self.asset_token {
            Ok((r_a, r_v, self.deposit_fee))
        } else if base_token == self.vault_token {
            Ok((r_v, r_a, self.withdraw_fee))
        } else {
            Err(AMMError::IncompatibleToken)
        }?;
        let numerator = reserve_out * F64_FEE_ONE;
        let denominator = reserve_in * (U32_FEE_ONE - fee) as f64;
        Ok(numerator / denominator)
    }

    fn simulate_swap(
        &self,
        base_token: Address,
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if self.vault_token == base_token {
            Ok(self.get_amount_out(amount_in, self.vault_reserve, self.asset_reserve)?)
        } else {
            Ok(self.get_amount_out(amount_in, self.asset_reserve, self.vault_reserve)?)
        }
    }

    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if self.vault_token == base_token {
            let amount_out =
                self.get_amount_out(amount_in, self.vault_reserve, self.asset_reserve)?;

            self.vault_reserve -= amount_in;
            self.asset_reserve -= amount_out;

            Ok(amount_out)
        } else {
            let amount_out =
                self.get_amount_out(amount_in, self.asset_reserve, self.vault_reserve)?;

            self.asset_reserve += amount_in;
            self.vault_reserve += amount_out;

            Ok(amount_out)
        }
    }

    // TODO: clean up this function
    async fn init<T, N, P>(
        mut self,
        block_number: BlockId,
        provider: Arc<P>,
    ) -> Result<Self, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let deployer = IGetERC4626VaultDataBatchRequest::deploy_builder(
            provider,
            vec![self.vault_token.address],
        );
        let res = deployer.call_raw().block(block_number).await?;

        let data = <Vec<(
            Address,
            u16,
            Address,
            u16,
            U256,
            U256,
            U256,
            U256,
            U256,
            U256,
            U256,
            U256,
        )> as SolValue>::abi_decode(&res, false)?;
        let (
            vault_token,
            vault_token_dec,
            asset_token,
            asset_token_dec,
            vault_reserve,
            asset_reserve,
            deposit_fee_delta_1,
            deposit_fee_delta_2,
            deposit_no_fee,
            withdraw_fee_delta_1,
            withdraw_fee_delta_2,
            withdraw_no_fee,
        ) = if !data.is_empty() {
            data[0]
        } else {
            todo!("Handle error")
        };

        // If both deltas are zero, the fee is zero
        if deposit_fee_delta_1.is_zero() && deposit_fee_delta_2.is_zero() {
            self.deposit_fee = 0;

        // Assuming 18 decimals, if the delta of 1e20 is half the delta of 2e20, relative fee.
        // Delta / (amount without fee / 1,000,000) to give us the fee in basis points
        } else if deposit_fee_delta_1 * U256_2 == deposit_fee_delta_2 {
            self.deposit_fee = (deposit_fee_delta_1 / (deposit_no_fee / U256::from(10_000))).to();
        } else {
            todo!("Handle error")
        }

        // If both deltas are zero, the fee is zero
        if withdraw_fee_delta_1.is_zero() && withdraw_fee_delta_2.is_zero() {
            self.withdraw_fee = 0;
        // Assuming 18 decimals, if the delta of 1e20 is half the delta of 2e20, relative fee.
        // Delta / (amount without fee / 1,000,000) to give us the fee in basis points
        } else if withdraw_fee_delta_1 * U256::from(2) == withdraw_fee_delta_2 {
            self.withdraw_fee =
                (withdraw_fee_delta_1 / (withdraw_no_fee / U256::from(10_000))).to();
        } else {
            // If not a relative fee or zero, ignore vault
            return Err(ERC4626VaultError::NonRelativeOrZeroFee.into());
        }

        // if above does not error => populate the vault
        self.vault_token = Token::new_with_decimals(vault_token, vault_token_dec as u8);
        self.asset_token = Token::new_with_decimals(asset_token, asset_token_dec as u8);
        self.vault_reserve = vault_reserve;
        self.asset_reserve = asset_reserve;

        Ok(self)
    }
}

// TODO: swap calldata
impl ERC4626Vault {
    // Returns a new, unsynced ERC4626 vault
    pub fn new(address: Address) -> Self {
        Self {
            vault_token: address.into(),
            ..Default::default()
        }
    }

    pub fn get_amount_out(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
    ) -> Result<U256, AMMError> {
        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        if self.vault_reserve.is_zero() {
            return Ok(amount_in);
        }

        let fee = if reserve_in == self.vault_reserve {
            self.withdraw_fee
        } else {
            self.deposit_fee
        };

        if reserve_in.is_zero() || U32_FEE_ONE - fee == 0 {
            return Err(ERC4626VaultError::DivisionByZero.into());
        }

        // TODO: support virtual offset?
        // TODO: guessing this new fee calculation is more accurate but not sure
        let fee_num = U32_FEE_ONE - fee;
        let numerator = amount_in * reserve_out * U256::from(fee_num);
        let denominator = reserve_in * U256_FEE_ONE;
        Ok(numerator / denominator)
    }

    pub async fn get_reserves<T, N, P>(
        &self,
        provider: P,
        block_number: BlockId,
    ) -> Result<(U256, U256), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Clone,
    {
        let vault = IERC4626Vault::new(self.vault_token.address, provider);

        let total_assets = vault.totalAssets().block(block_number).call().await?._0;

        let total_supply = vault.totalSupply().block(block_number).call().await?._0;

        Ok((total_supply, total_assets))
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{address, Address, U256};
    use float_cmp::assert_approx_eq;

    use crate::amms::{amm::AutomatedMarketMaker, Token};

    use super::ERC4626Vault;

    fn get_test_vault(vault_reserve: u128, asset_reserve: u128) -> ERC4626Vault {
        ERC4626Vault {
            vault_token: Token {
                address: address!("163538E22F4d38c1eb21B79939f3d2ee274198Ff"),
                decimals: 18,
            },
            asset_token: Token {
                address: address!("6B175474E89094C44Da98b954EedeAC495271d0F"),
                decimals: 6,
            },
            vault_reserve: U256::from(vault_reserve),
            asset_reserve: U256::from(asset_reserve),
            // ficticious fees
            deposit_fee: 1000,
            withdraw_fee: 5000,
        }
    }

    #[test]
    fn test_calculate_price_varying_decimals() {
        let vault = get_test_vault(501910315708981197269904, 505434849031);

        let price_v_for_a = vault
            .calculate_price(vault.vault_token.address, Address::default())
            .unwrap();
        let price_a_for_v = vault
            .calculate_price(vault.asset_token.address, Address::default())
            .unwrap();

        assert_approx_eq!(f64, price_v_for_a, 1.012082650516304962229139433, ulps = 4);
        assert_approx_eq!(f64, price_a_for_v, 0.9940207514393293696121269615, ulps = 4);
    }

    #[test]
    fn test_calculate_price_zero_reserve() {
        let vault = get_test_vault(0, 0);

        let price_v_for_a = vault
            .calculate_price(vault.vault_token.address, Address::default())
            .unwrap();
        let price_a_for_v = vault
            .calculate_price(vault.asset_token.address, Address::default())
            .unwrap();

        assert_eq!(price_v_for_a, 1.0);
        assert_eq!(price_a_for_v, 1.0);
    }

    #[test]
    fn test_simulate_swap() {
        let vault = get_test_vault(501910315708981197269904, 505434849031054568651911);

        let assets_out = vault
            .simulate_swap(
                vault.vault_token.address,
                vault.asset_token.address,
                U256::from(3000000000000000000_u128),
            )
            .unwrap();
        let shares_out = vault
            .simulate_swap(
                vault.asset_token.address,
                vault.vault_token.address,
                U256::from(3000000000000000000_u128),
            )
            .unwrap();

        assert_eq!(assets_out, U256::from(3005961378232538995_u128));
        assert_eq!(shares_out, U256::from(2976101111871285139_u128));
    }
}
