pub mod batch_request;
pub mod bmath;
pub mod error;
pub mod factory;

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
use rug::{float::Round, Float};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BalancerError {
    #[error("Base token does not exist")]
    BaseTokenDoesNotExist,
    #[error("Quote token does not exist")]
    QuoteTokenDoesNotExist,
    #[error("Division by zero")]
    DivZero,
    #[error("Error during division")]
    DivInternal,
    #[error("Addition overflow")]
    AddOverflow,
    #[error("Subtraction underflow")]
    SubUnderflow,
    #[error("Multiplication overflow")]
    MulOverflow,
}


use super::{amm::AutomatedMarketMaker, consts::{BONE, MPFR_T_PRECISION}, error::AMMError, float::u256_to_float};

sol! {
    // TODO: Add Liquidity Provision event's to sync stream.
    #[derive(Debug, PartialEq, Eq)]
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
        function calcOutGivenIn(
            uint tokenBalanceIn,
            uint tokenWeightIn,
            uint tokenBalanceOut,
            uint tokenWeightOut,
            uint tokenAmountIn,
            uint swapFee
        )
             external
            returns (uint);
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

    fn sync_events(&self) -> Vec<B256> {
        vec![IBPool::LOG_SWAP::SIGNATURE_HASH]
    }

    fn sync(&mut self, log: &Log) -> Result<(), AMMError> {
        self.sync_from_log(log.clone())
    }

    /// Returns a vector of tokens in the AMM.
    fn tokens(&self) -> Vec<Address> {
        self.tokens.clone()
    }

    /// Calculates a f64 representation of base token price in the AMM. This is a "tax inclusive" spot approximation.
    /// **********************************************************************************************
    /// calcSpotPrice                                                                             //
    /// sP = spotPrice                                                                            //
    /// bI = tokenBalanceIn                ( bI / wI )         1                                  //
    /// bO = tokenBalanceOut         sP =  -----------  *  ----------                             //
    /// wI = tokenWeightIn                 ( bO / wO )     ( 1 - sF )                             //
    /// wO = tokenWeightOut                                                                       //
    /// sF = swapFee                                                                              //
    ///**********************************************************************************************/
    fn calculate_price(&self, base_token: Address, quote_token: Address) -> Result<f64, AMMError> {
        // Grab the indices of the tokens
        let base_token_index = self
            .tokens
            .iter()
            .position(|&r| r == base_token)
            .map_or_else(|| Err(BalancerError::BaseTokenDoesNotExist), Ok)?;
        let quote_token_index = self
            .tokens
            .iter()
            .position(|&r| r == quote_token)
            .map_or_else(|| Err(BalancerError::QuoteTokenDoesNotExist), Ok)?;
        let bone = u256_to_float(BONE)?;
        let norm_base = if self.decimals[base_token_index] < 18 {
            Float::with_val(
                MPFR_T_PRECISION,
                10_u64.pow(18 - self.decimals[base_token_index] as u32),
            )
        } else {
            Float::with_val(MPFR_T_PRECISION, 1)
        };
        let norm_quote = if self.decimals[quote_token_index] < 18 {
            Float::with_val(
                MPFR_T_PRECISION,
                10_u64.pow(18 - self.decimals[quote_token_index] as u32),
            )
        } else {
            Float::with_val(MPFR_T_PRECISION, 1)
        };

        let norm_weight_base = u256_to_float(self.weights[base_token_index])? / norm_base;
        let norm_weight_quote = u256_to_float(self.weights[quote_token_index])? / norm_quote;
        let balance_base = u256_to_float(self.liquidity[base_token_index])?;
        let balance_quote = u256_to_float(self.liquidity[quote_token_index])?;

        let dividend = (balance_quote / norm_weight_quote) * bone.clone();
        let divisor = (balance_base / norm_weight_base)
            * (bone - Float::with_val(MPFR_T_PRECISION, self.fee));
        let ratio = dividend / divisor;
        Ok(ratio.to_f64_round(Round::Nearest))
    }

    /// Locally simulates a swap in the AMM.
    ///
    /// Returns the amount received for `amount_in` of `token_in`.
    fn simulate_swap(
        &self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        let base_token_index = self
            .tokens
            .iter()
            .position(|&r| r == base_token)
            .map_or_else(
                || {
                    Err(BalancerError::BaseTokenDoesNotExist)
                },
                Ok,
            )?;
        let quote_token_index = self
            .tokens
            .iter()
            .position(|&r| r == quote_token)
            .map_or_else(
                || {
                    Err(BalancerError::QuoteTokenDoesNotExist)
                },
                Ok,
            )?;

        let base_token_balance = self.liquidity[base_token_index];
        let quote_token_balance = self.liquidity[quote_token_index];
        let base_token_weight = self.weights[base_token_index];
        let quote_token_weight = self.weights[quote_token_index];
        let swap_fee = U256::from(self.fee);
        Ok(bmath::calculate_out_given_in(
            base_token_balance,
            base_token_weight,
            quote_token_balance,
            quote_token_weight,
            amount_in,
            swap_fee,
        )?)
    }

    /// Locally simulates a swap in the AMM.
    /// Mutates the AMM state to the state of the AMM after swapping.
    /// Returns the amount received for `amount_in` of `token_in`.
    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        let base_token_index = self
            .tokens
            .iter()
            .position(|&r| r == base_token)
            .map_or_else(
                || {
                    Err(BalancerError::BaseTokenDoesNotExist)
                },
                Ok,
            )?;
        let quote_token_index = self
            .tokens
            .iter()
            .position(|&r| r == quote_token)
            .map_or_else(
                || {
                    Err(BalancerError::QuoteTokenDoesNotExist)
                },
                Ok,
            )?;

        let base_token_balance = self.liquidity[base_token_index];
        let quote_token_balance = self.liquidity[quote_token_index];
        let base_token_weight = self.weights[base_token_index];
        let quote_token_weight = self.weights[quote_token_index];
        let swap_fee = U256::from(self.fee);
        let out = bmath::calculate_out_given_in(
            base_token_balance,
            base_token_weight,
            quote_token_balance,
            quote_token_weight,
            amount_in,
            swap_fee,
        ).map_err(BalancerError::from)?;
        self.liquidity[base_token_index] = bmath::badd(base_token_balance, amount_in)?;
        self.liquidity[quote_token_index] = bmath::bsub(quote_token_balance, out)?;
        Ok(out)
    }
}

impl BalancerV2Pool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: Address,
        tokens: Vec<Address>,
        decimals: Vec<u8>,
        liquidity: Vec<U256>,
        weights: Vec<U256>,
        fee: u32,
    ) -> BalancerV2Pool {
        BalancerV2Pool {
            address,
            tokens,
            decimals,
            liquidity,
            weights,
            fee,
        }
    }

    /// Populates the AMM data via batched static calls.
    async fn populate_data<T, N, P>(
        &mut self,
        block_number: Option<u64>,
        provider: P,
    ) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Clone,
    {
        Ok(
            batch_request::get_balancer_v2_pool_data_batch_request(self, block_number, provider)
                .await?,
        )
    }

    /// Updates the AMM data from a log.
    #[instrument(skip(self), level = "debug")]
    fn sync_from_log(&mut self, log: Log) -> Result<(), AMMError> {
        let signature = log.topics()[0];

        if IBPool::LOG_SWAP::SIGNATURE_HASH == signature {
            self.sync_from_swap_log(log)?;
        } else {
            return Err(AMMError::UnrecognizedEventSignature(signature));
        }

        Ok(())
    }

    pub fn sync_from_swap_log(
        &mut self,
        log: Log,
    ) -> Result<alloy::primitives::Log<IBPool::LOG_SWAP>, AMMError> {
        let swap_event = IBPool::LOG_SWAP::decode_log(log.as_ref(), true)?;

        let token_in_index = self
            .tokens
            .iter()
            .position(|r| r == &swap_event.tokenIn)
            .unwrap();
        let token_out_index = self
            .tokens
            .iter()
            .position(|r| r == &swap_event.tokenOut)
            .unwrap();

        // Update the pool liquidity
        self.liquidity[token_in_index] += swap_event.tokenAmountIn;
        self.liquidity[token_out_index] -= swap_event.tokenAmountOut;

        tracing::debug!(?swap_event, address = ?self.address, liquidity = ?self.liquidity);

        Ok(swap_event)
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use alloy::{
        primitives::{address, U256},
        providers::ProviderBuilder,
    };

    #[tokio::test]
    pub async fn test_populate_data() {
        let mut balancer_v2_pool = super::BalancerV2Pool {
            address: address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"),
            ..Default::default()
        };
        let provider = Arc::new(
            ProviderBuilder::new().on_http(env!("ETHEREUM_RPC_ENDPOINT").parse().unwrap()),
        );
        balancer_v2_pool
            .populate_data(Some(20487793), provider.clone())
            .await
            .unwrap();
        println!("Balancer V2 Pool: {:?}", balancer_v2_pool);
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
        let provider = Arc::new(
            ProviderBuilder::new().on_http(env!("ETHEREUM_RPC_ENDPOINT").parse().unwrap()),
        );
        let mut balancer_v2_pool = super::BalancerV2Pool {
            address: address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"),
            ..Default::default()
        };
        balancer_v2_pool
            .populate_data(Some(20487793), provider.clone())
            .await
            .unwrap();

        let calculated = balancer_v2_pool
            .calculate_price(
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            )
            .unwrap();

        assert_eq!(calculated, 2662.153859723404_f64);
    }

    #[tokio::test]
    pub async fn test_simulate_swap() {
        let provider = Arc::new(
            ProviderBuilder::new().on_http(env!("ETHEREUM_RPC_ENDPOINT").parse().unwrap()),
        );
        let mut balancer_v2_pool = super::BalancerV2Pool {
            address: address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"),
            ..Default::default()
        };
        balancer_v2_pool
            .populate_data(Some(20487793), provider.clone())
            .await
            .unwrap();
        println!("Balancer V2 Pool: {:?}", balancer_v2_pool);

        // 1 ETH
        let amount_in = U256::from(10_u64.pow(18));
        let calculated = balancer_v2_pool
            .simulate_swap(
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                amount_in,
            )
            .unwrap();

        let b_pool_quoter = IBPoolInstance::new(
            address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"),
            provider.clone(),
        );

        let expected = b_pool_quoter
            .calcOutGivenIn(
                balancer_v2_pool.liquidity[0],
                balancer_v2_pool.weights[0],
                balancer_v2_pool.liquidity[1],
                balancer_v2_pool.weights[1],
                amount_in,
                U256::from(balancer_v2_pool.fee),
            )
            .call()
            .await
            .unwrap();

        assert_eq!(calculated, expected._0);
    }
}
