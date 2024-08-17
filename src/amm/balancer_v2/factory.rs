use std::sync::Arc;

use alloy::{
    network::Network,
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    amm::{factory::AutomatedMarketMakerFactory, AMM},
    errors::AMMError,
};

use super::batch_request;

sol! {
    /// Interface of the UniswapV2Pairß
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
        Ok(vec![])
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
        todo!()
    }

    /// Creates a new empty AMM from a log factory creation event.
    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, alloy::sol_types::Error> {
        todo!()
    }
}
