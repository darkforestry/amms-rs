use std::sync::Arc;

use alloy::{
    network::AnyNetwork,
    primitives::{Address, FixedBytes, B256, U256},
    providers::Provider,
    rpc::types::eth::Log,
    sol,
    sol_types::SolEvent,
    transports::Transport,
};
use async_trait::async_trait;

use crate::{
    amm::{factory::AutomatedMarketMakerFactory, AMM},
    errors::AMMError,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{batch_request, UniswapV2Pool};

sol! {
    /// Interface of the UniswapV2Factory contract
    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract IUniswapV2FactorySol {
        event PairCreated(address indexed token0, address indexed token1, address pair, uint256 index);
        function getPair(address tokenA, address tokenB) external view returns (address pair);
        function allPairs(uint256 index) external view returns (address pair);
        function allPairsLength() external view returns (uint256 length);
    }
}

pub const PAIR_CREATED_EVENT_SIGNATURE: B256 = FixedBytes([
    13, 54, 72, 189, 15, 107, 168, 1, 52, 163, 59, 169, 39, 90, 197, 133, 217, 211, 21, 240, 173,
    131, 85, 205, 222, 253, 227, 26, 250, 40, 208, 233,
]);

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct UniswapV2Factory {
    pub address: Address,
    pub creation_block: u64,
    pub fee: u32,
}

impl UniswapV2Factory {
    pub fn new(address: Address, creation_block: u64, fee: u32) -> UniswapV2Factory {
        UniswapV2Factory {
            address,
            creation_block,
            fee,
        }
    }

    pub async fn get_all_pairs_via_batched_calls<
        T: Transport + Clone,
        P: Provider<T, AnyNetwork>,
    >(
        &self,
        provider: Arc<P>,
    ) -> Result<Vec<AMM>, AMMError> {
        let factory = IUniswapV2FactorySol::new(self.address, provider.clone());

        let IUniswapV2FactorySol::allPairsLengthReturn {
            length: pairs_length,
        } = factory.allPairsLength().call().await?;

        let mut pairs = vec![];
        // NOTE: max batch size for this call until codesize is too large
        let step = 766;
        let mut idx_from = U256::ZERO;
        let mut idx_to = if step > pairs_length.to::<usize>() {
            pairs_length
        } else {
            U256::from(step)
        };

        for _ in (0..pairs_length.to::<usize>()).step_by(step) {
            pairs.append(
                &mut batch_request::get_pairs_batch_request(
                    self.address,
                    idx_from,
                    idx_to,
                    provider.clone(),
                )
                .await?,
            );

            idx_from = idx_to;

            if idx_to + U256::from(step) > pairs_length {
                idx_to = pairs_length - U256::from(1)
            } else {
                idx_to += U256::from(step);
            }
        }

        let mut amms = vec![];

        // Create new empty pools for each pair
        for addr in pairs {
            let amm = UniswapV2Pool {
                address: addr,
                ..Default::default()
            };

            amms.push(AMM::UniswapV2Pool(amm));
        }

        Ok(amms)
    }
}

#[async_trait]
impl AutomatedMarketMakerFactory for UniswapV2Factory {
    fn address(&self) -> Address {
        self.address
    }

    fn amm_created_event_signature(&self) -> B256 {
        PAIR_CREATED_EVENT_SIGNATURE
    }

    async fn new_amm_from_log<T: Transport + Clone, P: Provider<T, AnyNetwork>>(
        &self,
        log: Log,
        provider: Arc<P>,
    ) -> Result<AMM, AMMError> {
        let pair_created_event = IUniswapV2FactorySol::PairCreated::decode_log(log.as_ref(), true)?;
        Ok(AMM::UniswapV2Pool(
            UniswapV2Pool::new_from_address(pair_created_event.pair, self.fee, provider).await?,
        ))
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, alloy::sol_types::Error> {
        let pair_created_event = IUniswapV2FactorySol::PairCreated::decode_log(log.as_ref(), true)?;

        Ok(AMM::UniswapV2Pool(UniswapV2Pool {
            address: pair_created_event.pair,
            token_a: pair_created_event.token0,
            token_b: pair_created_event.token1,
            token_a_decimals: 0,
            token_b_decimals: 0,
            reserve_0: 0,
            reserve_1: 0,
            fee: 0,
        }))
    }

    #[instrument(skip(self, middleware) level = "debug")]
    async fn get_all_amms<T: Transport + Clone, P: Provider<T, AnyNetwork>>(
        &self,
        _to_block: Option<u64>,
        middleware: Arc<P>,
        _step: u64,
    ) -> Result<Vec<AMM>, AMMError> {
        self.get_all_pairs_via_batched_calls(middleware).await
    }

    async fn populate_amm_data<T: Transport + Clone, P: Provider<T, AnyNetwork>>(
        &self,
        amms: &mut [AMM],
        _block_number: Option<u64>,
        middleware: Arc<P>,
    ) -> Result<(), AMMError> {
        let step = 127; //Max batch size for call
        for amm_chunk in amms.chunks_mut(step) {
            batch_request::get_amm_data_batch_request(amm_chunk, middleware.clone()).await?;
        }
        Ok(())
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }
}
