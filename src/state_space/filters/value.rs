use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use alloy::{
    primitives::{Address, U256},
    sol,
};
use eyre::Result;
use tokio::runtime::Handle;
use WethValueInPools::{PoolInfo, PoolInfoReturn, PoolType};

use crate::amms::amm::{AutomatedMarketMaker, AMM};

use super::AMMFilter;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IWethValueInPoolsBatchRequest,
    "src/state_space/filters/abi/WethValueInPoolsBatchRequest.json"
}

pub struct ValueFilter<const CHUNK_SIZE: usize, T, N, P> {
    pub uniswap_v2_factory: Address,
    pub uniswap_v3_factory: Address,
    pub weth: Address,
    pub min_weth_threshold: U256,
    pub provider: Arc<P>,
    phantom: PhantomData<(T, N)>,
}

impl<const CHUNK_SIZE: usize, T, N, P> ValueFilter<CHUNK_SIZE, T, N, P> {
    pub fn new(
        uniswap_v2_factory: Address,
        uniswap_v3_factory: Address,
        weth: Address,
        min_weth_threshold: U256,
        provider: Arc<P>,
    ) -> Self {
        Self {
            uniswap_v2_factory,
            uniswap_v3_factory,
            weth,
            min_weth_threshold,
            provider,
            phantom: PhantomData,
        }
    }

    pub async fn get_weth_value_in_pools(
        &self,
        pools: Vec<PoolInfo>,
    ) -> HashMap<Address, PoolInfoReturn> {
        // TODO: Deploy WethValueInPoolsBatchRequest contract
        // AbiDecode returns
        HashMap::new()
    }
}

impl<const CHUNK_SIZE: usize, T, N, P> AMMFilter for ValueFilter<CHUNK_SIZE, T, N, P> {
    fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>> {
        let pool_infos = amms
            .iter()
            .cloned()
            .map(|amm| {
                let pool_address = amm.address();
                // TODO FIXME: Need to update this when we have balancer/v3 support
                let pool_type = match amm {
                    AMM::UniswapV2Pool(_) => 1,
                };

                PoolInfo {
                    poolType: pool_type,
                    poolAddress: pool_address,
                }
            })
            .collect::<Vec<_>>();

        let mut pool_info_returns = HashMap::new();
        pool_infos.chunks(CHUNK_SIZE).for_each(|chunk| {
            let rt_handle = Handle::current();
            pool_info_returns
                .extend(rt_handle.block_on(self.get_weth_value_in_pools(chunk.to_vec())));
        });

        let filtered_amms = amms
            .into_iter()
            .filter(|amm| {
                let pool_address = amm.address();
                pool_info_returns
                    .get(&pool_address)
                    .map_or(false, |pool_info_return| {
                        pool_info_return.wethValue > self.min_weth_threshold
                    })
            })
            .collect::<Vec<_>>();
        Ok(filtered_amms)
    }
}
