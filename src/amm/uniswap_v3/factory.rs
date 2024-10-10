use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use alloy::{
    network::Network,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::eth::{Filter, Log},
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use async_trait::async_trait;
use futures::{stream::FuturesOrdered, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    amm::{factory::AutomatedMarketMakerFactory, AutomatedMarketMaker, AMM},
    errors::{AMMError, EventLogError},
};

use super::{batch_request, IUniswapV3Pool, UniswapV3Pool};

sol! {
    /// Interface of the UniswapV3Factory contract
    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract IUniswapV3Factory {
        event PoolCreated(address indexed token0, address indexed token1, uint24 indexed fee, int24 tickSpacing, address pool);
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
        function parameters() returns (address, address, uint24, int24);
        function feeAmountTickSpacing(uint24) returns (int24);
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UniswapV3Factory {
    pub address: Address,
    pub creation_block: u64,
}

#[async_trait]
impl AutomatedMarketMakerFactory for UniswapV3Factory {
    fn address(&self) -> Address {
        self.address
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }

    fn amm_created_event_signature(&self) -> B256 {
        IUniswapV3Factory::PoolCreated::SIGNATURE_HASH
    }

    async fn new_amm_from_log<T, N, P>(&self, log: Log, provider: Arc<P>) -> Result<AMM, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        if let Some(block_number) = log.block_number {
            let pool_created_filter = IUniswapV3Factory::PoolCreated::decode_log(&log.inner, true)?;
            Ok(AMM::UniswapV3Pool(
                UniswapV3Pool::new_from_address(pool_created_filter.pool, Some(log.address()), block_number, provider)
                    .await?,
            ))
        } else {
            return Err(AMMError::BlockNumberNotFound);
        }
    }

    async fn get_all_amms<T, N, P>(
        &self,
        to_block: Option<u64>,
        provider: Arc<P>,
        step: u64,
    ) -> Result<Vec<AMM>, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        if let Some(block) = to_block {
            self.get_all_pools_from_logs(block, step, provider).await
        } else {
            return Err(AMMError::BlockNumberNotFound);
        }
    }

    #[instrument(skip(self, amms, provider) level = "debug")]
    async fn populate_amm_data<T, N, P>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        provider: Arc<P>,
    ) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        if let Some(block_number) = block_number {
            // Max batch size for call
            let step = 76;
            for amm_chunk in amms.chunks_mut(step) {
                batch_request::get_amm_data_batch_request(
                    amm_chunk,
                    block_number,
                    provider.clone(),
                )
                .await?;
            }
        } else {
            return Err(AMMError::BlockNumberNotFound);
        }

        Ok(())
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, alloy::sol_types::Error> {
        let pool_created_event = IUniswapV3Factory::PoolCreated::decode_log(&log.inner, true)?;

        Ok(AMM::UniswapV3Pool(UniswapV3Pool {
            address: pool_created_event.pool,
            factory_address: Some(log.address()),
            token_a: pool_created_event.token0,
            token_b: pool_created_event.token1,
            token_a_decimals: 0,
            token_b_decimals: 0,
            fee: pool_created_event.fee.to::<u32>(),
            liquidity: 0,
            sqrt_price: U256::ZERO,
            tick_spacing: 0,
            tick: 0,
            tick_bitmap: HashMap::new(),
            ticks: HashMap::new(),
        }))
    }
}

impl UniswapV3Factory {
    pub fn new(address: Address, creation_block: u64) -> UniswapV3Factory {
        UniswapV3Factory {
            address,
            creation_block,
        }
    }

    // Function to get all pair created events for a given Dex factory address and sync pool data
    pub async fn get_all_pools_from_logs<T, N, P>(
        self,
        to_block: u64,
        step: u64,
        provider: Arc<P>,
    ) -> Result<Vec<AMM>, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        // Unwrap can be used here because the creation block was verified within `Dex::new()`
        let mut from_block = self.creation_block;
        let mut aggregated_amms: HashMap<Address, AMM> = HashMap::new();
        let mut ordered_logs: BTreeMap<u64, Vec<Log>> = BTreeMap::new();
        let mut futures = FuturesOrdered::new();

        while from_block < to_block {
            let provider = provider.clone();

            let mut target_block = from_block + step - 1;
            if target_block > to_block {
                target_block = to_block;
            }

            futures.push_back(async move {
                provider
                    .get_logs(
                        &Filter::new()
                            .event_signature(vec![
                                IUniswapV3Factory::PoolCreated::SIGNATURE_HASH,
                                IUniswapV3Pool::Burn::SIGNATURE_HASH,
                                IUniswapV3Pool::Mint::SIGNATURE_HASH,
                            ])
                            .from_block(from_block)
                            .to_block(target_block),
                    )
                    .await
            });

            from_block += step;
        }

        // TODO: this could be more dry since we use this in another place
        while let Some(result) = futures.next().await {
            let logs = result.map_err(AMMError::TransportError)?;

            for log in logs {
                if let Some(log_block_number) = log.block_number {
                    if let Some(log_group) = ordered_logs.get_mut(&log_block_number) {
                        log_group.push(log);
                    } else {
                        ordered_logs.insert(log_block_number, vec![log]);
                    }
                } else {
                    return Err(EventLogError::LogBlockNumberNotFound)?;
                }
            }
        }

        for (_, log_group) in ordered_logs {
            for log in log_group {
                let event_signature = log.topics()[0];

                //If the event sig is the pool created event sig, then the log is coming from the factory
                if event_signature == IUniswapV3Factory::PoolCreated::SIGNATURE_HASH {
                    if log.address() == self.address {
                        let mut new_pool = self.new_empty_amm_from_log(log)?;
                        if let AMM::UniswapV3Pool(ref mut pool) = new_pool {
                            pool.tick_spacing = pool.get_tick_spacing(provider.clone()).await?;
                        }

                        aggregated_amms.insert(new_pool.address(), new_pool);
                    }
                } else if event_signature == IUniswapV3Pool::Burn::SIGNATURE_HASH {
                    //If the event sig is the BURN_EVENT_SIGNATURE log is coming from the pool
                    if let Some(AMM::UniswapV3Pool(pool)) = aggregated_amms.get_mut(&log.address())
                    {
                        pool.sync_from_burn_log(log)?;
                    }
                } else if event_signature == IUniswapV3Pool::Mint::SIGNATURE_HASH {
                    if let Some(AMM::UniswapV3Pool(pool)) = aggregated_amms.get_mut(&log.address())
                    {
                        pool.sync_from_mint_log(log)?;
                    }
                }
            }
        }

        Ok(aggregated_amms.into_values().collect::<Vec<AMM>>())
    }
}
