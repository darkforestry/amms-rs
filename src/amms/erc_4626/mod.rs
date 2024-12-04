use alloy::{
    primitives::{Address, B256, U256},
    rpc::types::Log,
};
use serde::{Deserialize, Serialize};

use super::{amm::AutomatedMarketMaker, consts::U256_10000, error::AMMError};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ERC4626Vault {
    /// Token received from depositing, i.e. shares token
    pub vault_token: Address,
    pub vault_token_decimals: u8,
    /// Token received from withdrawing, i.e. underlying token
    pub asset_token: Address,
    pub asset_token_decimals: u8,
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
        self.vault_token
    }

    fn sync_events(&self) -> Vec<B256> {
        todo!()
    }

    fn sync(&mut self, log: &Log) -> Result<(), AMMError> {
        todo!()
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.vault_token, self.asset_token]
    }

    fn calculate_price(&self, base_token: Address, quote_token: Address) -> Result<f64, AMMError> {
        todo!()
    }

    fn simulate_swap(
        &self,
        base_token: Address,
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if self.vault_token == base_token {
            Ok(self.get_amount_out(amount_in, self.vault_reserve, self.asset_reserve))
        } else {
            Ok(self.get_amount_out(amount_in, self.asset_reserve, self.vault_reserve))
        }
    }

    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        todo!()
    }
}

// TODO: swap calldata

impl ERC4626Vault {
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

        amount_in * reserve_out / reserve_in * U256::from(10000 - fee) / U256_10000
    }
}
