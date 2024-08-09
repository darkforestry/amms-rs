pub mod batch_request;
pub mod factory;

use std::sync::Arc;

use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::Log,
    transports::Transport,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::errors::{AMMError, ArithmeticError, EventLogError, SwapSimulationError};

use super::AutomatedMarketMaker;

#[derive(Debug, Serialize, Deserialize)]
pub struct BalancerV2Pool {
    /// The Pool Address.
    address: Address,
    /// The Pool Tokens.
    tokens: Vec<Address>,
    /// The token decimals indexed by token.
    decimals: Vec<u8>,
    /// The Pool Weights indexed by token.
    liquidity: Vec<U256>,
    /// The Swap Fee on the Pool.
    fee: u32,
}

#[async_trait]
impl AutomatedMarketMaker for BalancerV2Pool {
    /// Returns the address of the AMM.
    fn address(&self) -> Address {
        self.address
    }

    /// Syncs the AMM data on chain via batched static calls.
    #[instrument(skip(self, provider), level = "debug")]
    async fn sync<T, N, P>(&mut self, provider: Arc<P>) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        todo!("Implement sync for BalancerPool")
    }

    /// Returns the vector of event signatures subscribed to when syncing the AMM.
    fn sync_on_event_signatures(&self) -> Vec<B256> {
        todo!()
    }

    /// Returns a vector of tokens in the AMM.
    fn tokens(&self) -> Vec<Address> {
        self.tokens.clone()
    }

    /// Calculates a f64 representation of base token price in the AMM.
    fn calculate_price(
        &self,
        base_token: Address,
        quote_token: Address,
    ) -> Result<f64, ArithmeticError> {
        todo!("Implement calculate_price for BalancerPool")
    }

    /// Updates the AMM data from a log.
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
        todo!("Implement sync_from_log for BalancerPool")
    }

    /// Populates the AMM data via batched static calls.
    async fn populate_data<T, N, P>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<P>,
    ) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        todo!("Implement populate_data for BalancerPool")
    }

    /// Locally simulates a swap in the AMM.
    ///
    /// Returns the amount received for `amount_in` of `token_in`.
    fn simulate_swap(
        &self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        todo!("Implement simulate_swap for BalancerPool")
    }

    /// Locally simulates a swap in the AMM.
    /// Mutates the AMM state to the state of the AMM after swapping.
    /// Returns the amount received for `amount_in` of `token_in`.
    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        todo!("Implement simulate_swap_mut for BalancerPool")
    }
}
