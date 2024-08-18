pub mod batch_request;
mod bmath;
pub mod factory;

use std::sync::Arc;

use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::errors::{AMMError, ArithmeticError, EventLogError, SwapSimulationError};

use super::AutomatedMarketMaker;

sol! {
    contract IBPool {
        event LOG_SWAP(
            address indexed caller,
            address indexed tokenIn,
            address indexed tokenOut,
            uint256         tokenAmountIn,
            uint256         tokenAmountOut
        );
    }
}
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BalancerV2Pool {
    /// The Pool Address.
    address: Address,
    /// The Pool Tokens.
    tokens: Vec<Address>,
    /// The token decimals indexed by token.
    decimals: Vec<u8>,
    /// The Pool Liquidity indexed by token.
    liquidity: Vec<U256>,
    /// The Pool Weights indexed by token.
    weights: Vec<U256>,
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
        // Tokens can change, so we are saving a request here.
        self.populate_data(None, provider).await
    }

    /// Returns the vector of event signatures subscribed to when syncing the AMM.
    fn sync_on_event_signatures(&self) -> Vec<B256> {
        vec![IBPool::LOG_SWAP::SIGNATURE_HASH]
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
        // https://github.com/balancer/balancer-core/blob/f4ed5d65362a8d6cec21662fb6eae233b0babc1f/contracts/BMath.sol#L28
        todo!("Implement calculate_price for BalancerPool")
    }

    /// Updates the AMM data from a log.
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
        let signature = log.topics()[0];
        if IBPool::LOG_SWAP::SIGNATURE_HASH == signature {
            let swap_event = IBPool::LOG_SWAP::decode_log(log.as_ref(), true)?;
            let token_in = swap_event.tokenIn;
            let token_out = swap_event.tokenOut;

            let token_in_index = self.tokens.iter().position(|&r| r == token_in).unwrap();
            let token_out_index = self.tokens.iter().position(|&r| r == token_out).unwrap();

            // Update the pool liquidity
            self.liquidity[token_in_index] += swap_event.tokenAmountIn;
            self.liquidity[token_out_index] -= swap_event.tokenAmountOut;
        }

        Ok(())
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
        Ok(
            batch_request::get_balancer_v2_pool_data_batch_request(self, block_number, middleware)
                .await?,
        )
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
        // https://github.com/balancer/balancer-core/blob/f4ed5d65362a8d6cec21662fb6eae233b0babc1f/contracts/BPool.sol#L423
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
        // https://github.com/balancer/balancer-core/blob/f4ed5d65362a8d6cec21662fb6eae233b0babc1f/contracts/BPool.sol#L423
        todo!("Implement simulate_swap_mut for BalancerPool")
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use alloy::{
        primitives::{address, U256},
        providers::ProviderBuilder,
    };

    use crate::amm::AutomatedMarketMaker;

    #[tokio::test]
    pub async fn test_populate_data() {
        let mut balancer_v2_pool = super::BalancerV2Pool {
            address: address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"),
            ..Default::default()
        };
        let provider =
            Arc::new(ProviderBuilder::new().on_http(env!("ETHEREUM_PROVIDER").parse().unwrap()));
        balancer_v2_pool
            .populate_data(Some(20487793), provider.clone())
            .await
            .unwrap();

        assert_eq!(
            balancer_v2_pool.tokens,
            vec![
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
            ]
        );
        assert_eq!(balancer_v2_pool.decimals, vec![18, 6]);
        assert_eq!(
            balancer_v2_pool.weights,
            vec![
                U256::from_str("25000000000000000000").unwrap(),
                U256::from_str("25000000000000000000").unwrap()
            ]
        );
        assert_eq!(balancer_v2_pool.fee, 640942080);
    }
}
