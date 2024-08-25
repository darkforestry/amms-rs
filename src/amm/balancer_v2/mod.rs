pub mod batch_request;
mod bmath;
pub mod factory;

use std::sync::Arc;

use alloy::{
    network::Network,
    primitives::{ruint::BaseConvertError, Address, B256, U256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use async_trait::async_trait;
use bmath::u256_to_float;
use rug::{float::Round, Float};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::errors::{AMMError, ArithmeticError, EventLogError, SwapSimulationError};

use super::{
    consts::{BONE, U256_10},
    AutomatedMarketMaker,
};

sol! {
    // TODO: Add Liquidity Provision event's to sync stream.
    #[sol(rpc)]
    contract IBPool {
        event LOG_SWAP(
            address indexed caller,
            address indexed tokenIn,
            address indexed tokenOut,
            uint256         tokenAmountIn,
            uint256         tokenAmountOut
        );
        function getSpotPrice(address tokenIn, address tokenOut) external returns (uint256);
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
    // export function _spotPriceAfterSwapExactTokenInForTokenOut(
    //     amount: OldBigNumber,
    //     poolPairData: WeightedPoolPairData
    // ): OldBigNumber {
    //     const Bi = parseFloat(
    //         formatFixed(poolPairData.balanceIn, poolPairData.decimalsIn)
    //     );
    //     const Bo = parseFloat(
    //         formatFixed(poolPairData.balanceOut, poolPairData.decimalsOut)
    //     );
    //     const wi = parseFloat(formatFixed(poolPairData.weightIn, 18));
    //     const wo = parseFloat(formatFixed(poolPairData.weightOut, 18));
    //     const Ai = amount.toNumber();
    //     const f = parseFloat(formatFixed(poolPairData.swapFee, 18));
    //     return bnum(
    //         -(
    //             (Bi * wo) /
    //             (Bo * (-1 + f) * (Bi / (Ai + Bi - Ai * f)) ** ((wi + wo) / wo) * wi)
    //         )
    //     );
    // }
    /// Calculates a f64 representation of base token price in the AMM.
    /// **********************************************************************************************
    /// calcSpotPrice                                                                             //
    /// sP = spotPrice                                                                            //
    /// bI = tokenBalanceIn                ( bI / wI )         1                                  //
    /// bO = tokenBalanceOut         sP =  -----------  *  ----------                             //
    /// wI = tokenWeightIn                 ( bO / wO )     ( 1 - sF )                             //
    /// wO = tokenWeightOut                                                                       //
    /// sF = swapFee                                                                              //
    ///**********************************************************************************************/
    fn calculate_price(
        &self,
        base_token: Address,
        quote_token: Address,
    ) -> Result<f64, ArithmeticError> {
        // Grab the indices of the tokens
        let base_token_index = self
            .tokens
            .iter()
            .position(|&r| r == base_token)
            .expect("Base token not found");
        let quote_token_index = self
            .tokens
            .iter()
            .position(|&r| r == quote_token)
            .expect("Quote token not found");

        let decimals_base = U256::from(self.decimals[base_token_index]);
        let weight_base = self.weights[base_token_index];
        let weight_quote = self.weights[quote_token_index];
        let balance_base = self.liquidity[base_token_index];
        let balance_quote = self.liquidity[quote_token_index];
        // This is an after tax approximation of the spot price.
        let epsilon = U256::from(10).pow(U256::from(12));

        let out = u256_to_float(bmath::calculate_out_given_in(
            balance_base,
            weight_base,
            balance_quote,
            weight_quote,
            epsilon,
            U256::from(self.fee),
        ));

        let decimal_factor =
            self.decimals[base_token_index] as i32 - self.decimals[quote_token_index] as i32;
        let mut ratio = out / u256_to_float(epsilon);
        let factor = u256_to_float(U256_10.pow(U256::from(decimal_factor.abs())));
        ratio = if decimal_factor < 0 {
            ratio / factor
        } else {
            ratio * factor
        };

        Ok(ratio.to_f64_round(Round::Nearest))
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
        eips::BlockId,
        primitives::{address, U256},
        providers::ProviderBuilder,
    };

    use crate::amm::AutomatedMarketMaker;

    use super::IBPool;

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

    #[tokio::test]
    pub async fn test_calculate_price() {
        let provider =
            Arc::new(ProviderBuilder::new().on_http(env!("ETHEREUM_PROVIDER").parse().unwrap()));
        let mut balancer_v2_pool = super::BalancerV2Pool {
            address: address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"),
            ..Default::default()
        };
        balancer_v2_pool
            .populate_data(Some(20487793), provider.clone())
            .await
            .unwrap();
        println!("Balancer V2 Pool: {:?}", balancer_v2_pool);
        let balancer_v2_pool_instance = IBPool::new(balancer_v2_pool.address, provider.clone());
        let expected = balancer_v2_pool_instance
            .getSpotPrice(
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            )
            .block(BlockId::from(20487793))
            .call()
            .await
            .unwrap()
            ._0;
        println!("USDC Balance: {:?}", balancer_v2_pool.liquidity[1]);
        println!("WETH Balance: {:?}", balancer_v2_pool.liquidity[0]);
        println!("Expected: {:?}", expected);
        let calculated = balancer_v2_pool
            .calculate_price(
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            )
            .unwrap();
        println!("Calculated: {:?}", calculated);
    }
}
