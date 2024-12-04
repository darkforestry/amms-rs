use super::{
    amm::{AutomatedMarketMaker, AMM},
    consts::{U128_0X10000000000000000, U256_10000},
    error::AMMError,
    factory::{AutomatedMarketMakerFactory, DiscoverySync},
    uniswap_v2::{div_uu, q64_to_float},
};
use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, future::Future, sync::Arc};
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
        vec![self.vault_token, self.asset_token]
    }

    fn calculate_price(&self, base_token: Address, _quote_token: Address) -> Result<f64, AMMError> {
        Ok(q64_to_float(self.calculate_price_64_x_64(base_token)?)?)
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
        _quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        if self.vault_token == base_token {
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

    // TODO: Right now this will return a uv2 error, fix this
    pub fn calculate_price_64_x_64(&self, base_token: Address) -> Result<u128, AMMError> {
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct ERC4626Factory {
    // Block to start searching from for new vaults
    pub creation_block: u64,
}
impl DiscoverySync for ERC4626Factory {
    fn discover<T, N, P>(
        &self,
        to_block: u64,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        info!(
            target = "amms::erc_4626::discover",
            "Discovering all vaults"
        );
        async move { todo!() }
    }

    fn sync<T, N, P>(
        &self,
        amms: Vec<AMM>,
        to_block: u64,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        info!(target = "amms::erc_4626::sync", "Syncing all vaults");

        async move { todo!() }
    }
}

// TODO: update factory macro to impl into impl
impl AutomatedMarketMakerFactory for ERC4626Factory {
    type PoolVariant = ERC4626Vault;

    fn address(&self) -> Address {
        Address::default()
    }

    fn create_pool(&self, _log: Log) -> Result<AMM, AMMError> {
        Ok(ERC4626Vault::default().into())
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }

    fn pool_creation_event(&self) -> B256 {
        todo!()
    }
}
