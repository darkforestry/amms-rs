pub mod bmath;

use std::{collections::HashMap, future::Future, sync::Arc};

use alloy::{
    eips::BlockId,
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::{Filter, FilterSet, Log},
    sol,
    sol_types::{SolEvent, SolValue},
    transports::Transport,
};
use async_trait::async_trait;
use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use rug::{float::Round, Float};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

use super::{
    amm::{AutomatedMarketMaker, AMM},
    consts::{BONE, MPFR_T_PRECISION},
    error::AMMError,
    factory::{AutomatedMarketMakerFactory, DiscoverySync},
    float::u256_to_float,
    Token,
};

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

    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract IBFactory {
        event LOG_NEW_POOL(
            address indexed caller,
            address indexed pool
        );
    }
}

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetBalancerPoolDataBatchRequest,
    "contracts/out/GetBalancerPoolDataBatchRequest.sol/GetBalancerPoolDataBatchRequest.json"
);

#[derive(Error, Debug)]
pub enum BalancerError {
    #[error("Error initializing Balancer Pool")]
    InitializationError,
    #[error("Token in does not exist")]
    TokenInDoesNotExist,
    #[error("Token out does not exist")]
    TokenOutDoesNotExist,
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

// TODO: we could consider creating a "Token" struct that would store the decimals.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BalancerPool {
    /// The Pool Address.
    address: Address,
    // TODO:
    state: HashMap<Address, TokenPoolState>,
    /// The Swap Fee on the Pool.
    fee: u32,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TokenPoolState {
    pub liquidity: U256,
    pub weight: U256,
    pub token: Token,
}

impl AutomatedMarketMaker for BalancerPool {
    /// Returns the address of the AMM.
    fn address(&self) -> Address {
        self.address
    }

    fn sync_events(&self) -> Vec<B256> {
        vec![IBPool::LOG_SWAP::SIGNATURE_HASH]
    }

    fn sync(&mut self, log: &Log) -> Result<(), AMMError> {
        let signature = log.topics()[0];

        if IBPool::LOG_SWAP::SIGNATURE_HASH == signature {
            let swap_event = IBPool::LOG_SWAP::decode_log(log.as_ref(), false)?;

            self.state
                .get_mut(&swap_event.tokenIn)
                .ok_or(BalancerError::TokenInDoesNotExist)?
                .liquidity += swap_event.tokenAmountIn;

            self.state
                .get_mut(&swap_event.tokenOut)
                .ok_or(BalancerError::TokenOutDoesNotExist)?
                .liquidity += swap_event.tokenAmountOut;

            info!(
                target = "amm::balancer::sync",
                address = ?self.address,
                state = ?self.state, "Sync"
            );
        } else {
            return Err(AMMError::UnrecognizedEventSignature(signature));
        }

        Ok(())
    }

    /// Returns a vector of tokens in the AMM.
    fn tokens(&self) -> Vec<Address> {
        self.state.keys().cloned().collect()
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
        let token_in = self
            .state
            .get(&base_token)
            .ok_or(BalancerError::TokenInDoesNotExist)?;

        let token_out = self
            .state
            .get(&quote_token)
            .ok_or(BalancerError::TokenOutDoesNotExist)?;

        let bone = u256_to_float(BONE)?;
        let norm_base = if token_in.token.decimals < 18 {
            Float::with_val(
                MPFR_T_PRECISION,
                10_u64.pow(18 - token_in.token.decimals as u32),
            )
        } else {
            Float::with_val(MPFR_T_PRECISION, 1)
        };
        let norm_quote = if token_out.token.decimals < 18 {
            Float::with_val(
                MPFR_T_PRECISION,
                10_u64.pow(18 - token_out.token.decimals as u32),
            )
        } else {
            Float::with_val(MPFR_T_PRECISION, 1)
        };

        let norm_weight_base = u256_to_float(token_in.weight)? / norm_base;
        let norm_weight_quote = u256_to_float(token_out.weight)? / norm_quote;
        let balance_base = u256_to_float(token_in.liquidity)?;
        let balance_quote = u256_to_float(token_out.liquidity)?;

        let dividend = (balance_quote / norm_weight_quote) * bone.clone();
        let divisor = (balance_base / norm_weight_base)
            * (bone - Float::with_val(MPFR_T_PRECISION, self.fee));
        let ratio = dividend / divisor;
        Ok(ratio.to_f64_round(Round::Nearest))
    }

    /// Locally simulates a swap in the AMM.
    ///
    /// # Returns
    /// The amount received for `amount_in` of `token_in`.
    fn simulate_swap(
        &self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        let token_in = self
            .state
            .get(&base_token)
            .ok_or(BalancerError::TokenInDoesNotExist)?;

        let token_out = self
            .state
            .get(&quote_token)
            .ok_or(BalancerError::TokenOutDoesNotExist)?;

        Ok(bmath::calculate_out_given_in(
            token_in.liquidity,
            token_in.weight,
            token_out.liquidity,
            token_out.weight,
            amount_in,
            U256::from(self.fee),
        )?)
    }

    /// Locally simulates a swap in the AMM.
    /// Mutates the AMM state to the state of the AMM after swapping.
    ///
    /// # Returns
    /// The amount received for `amount_in` of `token_in`.
    fn simulate_swap_mut(
        &mut self,
        base_token: Address,
        quote_token: Address,
        amount_in: U256,
    ) -> Result<U256, AMMError> {
        let token_in = self
            .state
            .get(&base_token)
            .ok_or(BalancerError::TokenInDoesNotExist)?;

        let token_out = self
            .state
            .get(&quote_token)
            .ok_or(BalancerError::TokenOutDoesNotExist)?;

        let out = bmath::calculate_out_given_in(
            token_in.liquidity,
            token_in.weight,
            token_out.liquidity,
            token_out.weight,
            amount_in,
            U256::from(self.fee),
        )?;

        self.state.get_mut(&base_token).unwrap().liquidity += amount_in;
        self.state.get_mut(&quote_token).unwrap().liquidity -= out;

        Ok(out)
    }

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
        let deployer =
            IGetBalancerPoolDataBatchRequest::deploy_builder(provider, vec![self.address]);
        let res = deployer.block(block_number).call_raw().await?;

        let mut data =
            <Vec<(Vec<Address>, Vec<u16>, Vec<U256>, Vec<U256>, u32)> as SolValue>::abi_decode(
                &res, false,
            )?;
        let (tokens, decimals, liquidity, weights, fee) = if !data.is_empty() {
            data.remove(0)
        } else {
            return Err(BalancerError::InitializationError.into());
        };

        let token_state = tokens
            .into_iter()
            .zip(decimals)
            .zip(liquidity)
            .zip(weights)
            .map(|(((token, decimals), liquidity), weight)| {
                (
                    token,
                    TokenPoolState {
                        liquidity,
                        weight,
                        token: Token::new(token, decimals as u8),
                    },
                )
            })
            .collect::<HashMap<Address, TokenPoolState>>();

        self.state = token_state;
        self.fee = fee;

        Ok(self)
    }
}

impl BalancerPool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(address: Address) -> BalancerPool {
        BalancerPool {
            address,
            ..Default::default()
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BalancerFactory {
    pub address: Address,
    pub creation_block: u64,
}

#[async_trait]
impl AutomatedMarketMakerFactory for BalancerFactory {
    type PoolVariant = BalancerPool;

    /// Address of the factory contract
    fn address(&self) -> Address {
        self.address
    }

    /// Creates an unsynced pool from a creation log.
    fn create_pool(&self, log: Log) -> Result<AMM, AMMError> {
        let pool_data = IBFactory::LOG_NEW_POOL::decode_log(&log.inner, true)?;
        Ok(AMM::BalancerPool(BalancerPool {
            address: pool_data.pool,
            ..Default::default()
        }))
    }

    /// Returns the block number at which the factory was created.
    fn creation_block(&self) -> u64 {
        self.creation_block
    }

    /// Event signature that indicates when a new pool was created
    fn pool_creation_event(&self) -> B256 {
        IBFactory::LOG_NEW_POOL::SIGNATURE_HASH
    }
}

impl DiscoverySync for BalancerFactory {
    fn discover<T, N, P>(
        &self,
        to_block: BlockId,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        info!(
            target = "amms::balancer::discover",
            address = ?self.address,
            "Discovering all pools"
        );
        self.get_all_pools(to_block, provider)
    }

    fn sync<T, N, P>(
        &self,
        amms: Vec<AMM>,
        to_block: BlockId,
        provider: Arc<P>,
    ) -> impl Future<Output = Result<Vec<AMM>, AMMError>>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        info!(
            target = "amms::balancer::sync",
            address = ?self.address,
            "Syncing all pools"
        );
        Self::sync_all_pools(amms, to_block, provider)
    }
}

impl BalancerFactory {
    pub fn new(address: Address, creation_block: u64) -> BalancerFactory {
        BalancerFactory {
            address,
            creation_block,
        }
    }

    pub async fn get_all_pools<T, N, P>(
        &self,
        block_number: BlockId,
        provider: Arc<P>,
    ) -> Result<Vec<AMM>, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let disc_filter = Filter::new()
            .event_signature(FilterSet::from(vec![self.pool_creation_event()]))
            .address(vec![self.address()]);

        let sync_provider = provider.clone();
        let mut futures = FuturesUnordered::new();

        let sync_step = 100_000;
        let mut latest_block = self.creation_block;
        while latest_block < block_number.as_u64().unwrap_or_default() {
            let mut block_filter = disc_filter.clone();
            let from_block = latest_block;
            let to_block = (from_block + sync_step).min(block_number.as_u64().unwrap_or_default());

            block_filter = block_filter.from_block(from_block);
            block_filter = block_filter.to_block(to_block);

            let sync_provider = sync_provider.clone();

            futures.push(async move { sync_provider.get_logs(&block_filter).await });

            latest_block = to_block + 1;
        }

        let mut pools = vec![];
        while let Some(res) = futures.next().await {
            let logs = res?;

            for log in logs {
                pools.push(self.create_pool(log)?);
            }
        }

        Ok(pools)
    }

    pub async fn sync_all_pools<T, N, P>(
        amms: Vec<AMM>,
        block_number: BlockId,
        provider: Arc<P>,
    ) -> Result<Vec<AMM>, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let step = 120;
        let pairs = amms
            .iter()
            .chunks(step)
            .into_iter()
            .map(|chunk| chunk.map(|amm| amm.address()).collect())
            .collect::<Vec<Vec<Address>>>();

        let mut futures_unordered = FuturesUnordered::new();
        for group in pairs {
            let deployer = IGetBalancerPoolDataBatchRequest::deploy_builder(
                provider.clone(),
                amms.iter().map(|amm| amm.address()).collect(),
            );

            futures_unordered.push(async move {
                let res = deployer.call_raw().block(block_number).await?;

                let return_data = <Vec<(Vec<Address>, Vec<u16>, Vec<U256>, Vec<U256>, u32)> as SolValue>::abi_decode(
                    &res, false,
                )?;

                Ok::<(Vec<Address>, Vec<(Vec<Address>, Vec<u16>, Vec<U256>, Vec<U256>, u32)>), AMMError>((
                    group,
                    return_data,
                ))
            });
        }

        let mut amms = amms
            .into_iter()
            .map(|amm| (amm.address(), amm))
            .collect::<HashMap<_, _>>();

        while let Some(res) = futures_unordered.next().await {
            let (group, return_data) = res?;
            for (pool_data, pool_address) in return_data.iter().zip(group.iter()) {
                let amm = amms.get_mut(pool_address).unwrap();

                let AMM::BalancerPool(pool) = amm else {
                    panic!("Unexpected pool type")
                };

                let tokens = pool_data.0.clone();
                let decimals = pool_data.1.clone();
                let liquidity = pool_data.2.clone();
                let weights = pool_data.3.clone();

                let token_state = tokens
                    .into_iter()
                    .zip(decimals)
                    .zip(liquidity)
                    .zip(weights)
                    .map(|(((token, decimals), liquidity), weight)| {
                        (
                            token,
                            TokenPoolState {
                                liquidity,
                                weight,
                                token: Token::new(token, decimals as u8),
                            },
                        )
                    })
                    .collect::<HashMap<Address, TokenPoolState>>();

                pool.state = token_state;
                pool.fee = pool_data.4;
            }
        }

        let amms = amms
            .into_iter()
            .filter_map(|(_, amm)| {
                if amm.tokens().iter().any(|t| t.is_zero()) {
                    None
                } else {
                    Some(amm)
                }
            })
            .collect();

        Ok(amms)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use alloy::{
        primitives::{address, Address, U256},
        providers::ProviderBuilder,
    };
    use eyre::Ok;

    use crate::amms::{
        amm::AutomatedMarketMaker,
        balancer::{BalancerPool, IBPool::IBPoolInstance},
    };
    use crate::amms::{balancer::TokenPoolState, Token};

    #[tokio::test]
    pub async fn test_populate_data() -> eyre::Result<()> {
        let provider =
            Arc::new(ProviderBuilder::new().on_http(env!("ETHEREUM_PROVIDER").parse().unwrap()));

        let balancer_pool = BalancerPool::new(address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"))
            .init(20487793.into(), provider.clone())
            .await?;

        // Construct the expected state as a HashMap
        let expected_state: HashMap<Address, TokenPoolState> = vec![
            (
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                TokenPoolState {
                    liquidity: U256::from(1234567890000000000_u128),
                    weight: U256::from(25000000000000000000_u128),
                    token: Token::new(address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"), 18),
                },
            ),
            (
                address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                TokenPoolState {
                    liquidity: U256::from(987654321000000_u128),
                    weight: U256::from(25000000000000000000_u128),
                    token: Token::new(address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"), 6),
                },
            ),
        ]
        .into_iter()
        .collect::<HashMap<Address, TokenPoolState>>();

        // Compare the actual state with the expected state
        assert_eq!(
            balancer_pool.state, expected_state,
            "Balancer pool state mismatch"
        );

        // Validate the fee
        let expected_fee = 640942080;
        assert_eq!(
            balancer_pool.fee, expected_fee,
            "Fee does not match expected value"
        );

        Ok(())
    }

    #[tokio::test]
    pub async fn test_calculate_price() -> eyre::Result<()> {
        let provider =
            Arc::new(ProviderBuilder::new().on_http(env!("ETHEREUM_PROVIDER").parse().unwrap()));

        let balancer_pool = BalancerPool::new(address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"))
            .init(20487793.into(), provider.clone())
            .await?;

        let calculated = balancer_pool
            .calculate_price(
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            )
            .unwrap();

        assert_eq!(calculated, 2662.153859723404_f64);
        Ok(())
    }

    #[tokio::test]
    pub async fn test_simulate_swap() -> eyre::Result<()> {
        let provider = Arc::new(ProviderBuilder::new().on_http(env!("ETHEREUM_PROVIDER").parse()?));

        let balancer_pool = BalancerPool::new(address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"))
            .init(20487793.into(), provider.clone())
            .await?;

        println!("Balancer V2 Pool: {:?}", balancer_pool);

        let token_in = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let token_out = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");

        let amount_in = U256::from(10_u64.pow(18));
        let calculated = balancer_pool.simulate_swap(token_in, token_out, amount_in)?;

        let b_pool_quoter = IBPoolInstance::new(
            address!("8a649274E4d777FFC6851F13d23A86BBFA2f2Fbf"),
            provider.clone(),
        );

        let token_in = balancer_pool.state.get(&token_in).unwrap();
        let token_out = balancer_pool.state.get(&token_out).unwrap();

        let expected = b_pool_quoter
            .calcOutGivenIn(
                token_in.liquidity,
                token_in.weight,
                token_out.liquidity,
                token_out.weight,
                amount_in,
                U256::from(balancer_pool.fee),
            )
            .call()
            .await
            .unwrap();

        assert_eq!(calculated, expected._0);

        Ok(())
    }
}
