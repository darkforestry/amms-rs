use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use super::{AMMFilter, FilterStage};
use crate::amms::{
    amm::{AutomatedMarketMaker, AMM},
    error::AMMError,
};
use alloy::{
    network::Network,
    primitives::{Address, U256},
    providers::Provider,
    sol,
    sol_types::SolValue,
    transports::Transport,
};
use async_trait::async_trait;
use WethValueInPools::{PoolInfo, PoolInfoReturn};

sol! {
    #[sol(rpc)]
    WethValueInPoolsBatchRequest,
    "contracts/out/WethValueInPools.sol/WethValueInPoolsBatchRequest.json"
}

pub struct ValueFilter<const CHUNK_SIZE: usize, T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    pub uniswap_v2_factory: Address,
    pub uniswap_v3_factory: Address,
    pub weth: Address,
    pub min_weth_threshold: U256,
    pub provider: Arc<P>,
    phantom: PhantomData<(T, N)>,
}

impl<const CHUNK_SIZE: usize, T, N, P> ValueFilter<CHUNK_SIZE, T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
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
    ) -> Result<HashMap<Address, PoolInfoReturn>, AMMError> {
        let deployer = WethValueInPoolsBatchRequest::deploy_builder(
            self.provider.clone(),
            self.uniswap_v2_factory,
            self.uniswap_v3_factory,
            self.weth,
            pools,
        );

        let res = deployer.call_raw().await?;
        let return_data = <Vec<PoolInfoReturn> as SolValue>::abi_decode(&res, false)?;

        Ok(return_data
            .into_iter()
            .map(|pool_info| (pool_info.poolAddress, pool_info))
            .collect())
    }
}

#[async_trait]
impl<const CHUNK_SIZE: usize, T, N, P> AMMFilter for ValueFilter<CHUNK_SIZE, T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>, AMMError> {
        let pool_infos = amms
            .iter()
            .cloned()
            .map(|amm| {
                let pool_address = amm.address();
                // TODO FIXME: Need to update this when we have balancer/v3 support
                let pool_type = match amm {
                    AMM::UniswapV2Pool(_) => 0,
                    AMM::UniswapV3Pool(_) => 1,
                    AMM::ERC4626Vault(_) => todo!(),
                    AMM::BalancerPool(_) => todo!(),
                };

                PoolInfo {
                    poolType: pool_type,
                    poolAddress: pool_address,
                }
            })
            .collect::<Vec<_>>();

        let mut pool_info_returns = HashMap::new();
        let futs = pool_infos
            .chunks(CHUNK_SIZE)
            .map(|chunk| async { self.get_weth_value_in_pools(chunk.to_vec()).await })
            .collect::<Vec<_>>();

        let results = futures::future::join_all(futs).await;
        for result in results {
            pool_info_returns.extend(result?);
        }

        let filtered_amms = amms
            .into_iter()
            .filter(|amm| {
                let pool_address = amm.address();
                pool_info_returns
                    .get(&pool_address).is_some_and(|pool_info_return| {
                        pool_info_return.wethValue > self.min_weth_threshold
                    })
            })
            .collect::<Vec<_>>();
        Ok(filtered_amms)
    }

    fn stage(&self) -> FilterStage {
        FilterStage::Sync
    }
}
