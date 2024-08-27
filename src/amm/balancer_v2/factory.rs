use std::sync::Arc;

use alloy::{
    network::Network,
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::{Filter, Log},
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use async_trait::async_trait;
use futures::{stream::FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};

use crate::{
    amm::{factory::AutomatedMarketMakerFactory, AutomatedMarketMaker, AMM},
    errors::AMMError,
};

use super::{batch_request, BalancerV2Pool};

sol! {
    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract IBFactory {
        event LOG_NEW_POOL(
            address indexed caller,
            address indexed pool
        );
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BalancerV2Factory {
    pub address: Address,
    pub creation_block: u64,
}

#[async_trait]
impl AutomatedMarketMakerFactory for BalancerV2Factory {
    /// Returns the address of the factory.
    fn address(&self) -> Address {
        self.address
    }

    /// Gets all Pools from the factory created logs up to the `to_block` block number.
    ///
    /// Returns a vector of AMMs.
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
        let to = to_block.unwrap_or(provider.get_block_number().await?);
        Ok(self.get_all_pools_from_logs(to, step, provider).await?)
    }

    /// Populates all AMMs data via batched static calls.
    async fn populate_amm_data<T, N, P>(
        &self,
        amms: &mut [AMM],
        _block_number: Option<u64>,
        provider: Arc<P>,
    ) -> Result<(), AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        // Max batch size for call
        let step = 127;
        for amm_chunk in amms.chunks_mut(step) {
            batch_request::get_amm_data_batch_request(amm_chunk, provider.clone()).await?;
        }
        Ok(())
    }

    /// Returns the creation event signature for the factory.
    fn amm_created_event_signature(&self) -> B256 {
        IBFactory::LOG_NEW_POOL::SIGNATURE_HASH
    }

    /// Returns the block number at which the factory was created.
    fn creation_block(&self) -> u64 {
        self.creation_block
    }

    /// Creates a new AMM from a log factory creation event.
    ///
    /// Returns a AMM with data populated.
    async fn new_amm_from_log<T, N, P>(&self, log: Log, provider: Arc<P>) -> Result<AMM, AMMError>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
    {
        let mut pool = self.new_empty_amm_from_log(log)?;
        pool.populate_data(None, provider).await?;
        Ok(pool)
    }

    /// Creates a new empty AMM from a log factory creation event.
    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, alloy::sol_types::Error> {
        let pair_created_event = IBFactory::LOG_NEW_POOL::decode_log(log.as_ref(), true)?;
        let pool = BalancerV2Pool {
            address: pair_created_event.pool,
            ..Default::default()
        };
        Ok(AMM::BalancerV2Pool(pool))
    }
}

impl BalancerV2Factory {
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

        let mut futures = FuturesUnordered::new();
        let mut amms = vec![];
        while from_block < to_block {
            let provider = provider.clone();

            let mut target_block = from_block + step - 1;
            if target_block > to_block {
                target_block = to_block;
            }

            futures.push(async move {
                provider
                    .get_logs(
                        &Filter::new()
                            .event_signature(vec![IBFactory::LOG_NEW_POOL::SIGNATURE_HASH])
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
                amms.push(self.new_amm_from_log(log, provider.clone()).await?);
            }
        }

        Ok(amms)
    }
}
