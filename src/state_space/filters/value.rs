use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use crate::amms::amm::{AutomatedMarketMaker, AMM};
use alloy::{
    dyn_abi::{DynSolType, DynSolValue},
    network::Network,
    primitives::{Address, U256},
    providers::Provider,
    sol,
    transports::Transport,
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use WethValueInPools::{PoolInfo, PoolInfoReturn};

use super::AMMFilter;

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

impl TryFrom<&DynSolValue> for PoolInfoReturn {
    type Error = eyre::Error;
    fn try_from(value: &DynSolValue) -> Result<Self, Self::Error> {
        let tuple = value.as_tuple().ok_or(eyre!(
            "Expected tuple with 3 elements: (uint8, address, uint256) for PoolInfoReturn"
        ))?;
        let pool_type = tuple[0]
            .as_uint()
            .ok_or(eyre!("Failed to decode pool type"))?
            .0
            .to();
        let pool_address = tuple[1]
            .as_address()
            .ok_or(eyre!("Failed to decode pool address"))?;
        let weth_value = tuple[2]
            .as_uint()
            .ok_or(eyre!("Failed to decode weth value"))?
            .0;
        Ok(Self {
            poolType: pool_type,
            poolAddress: pool_address,
            wethValue: weth_value,
        })
    }
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
    ) -> Result<HashMap<Address, PoolInfoReturn>> {
        let deployer = WethValueInPoolsBatchRequest::deploy_builder(
            self.provider.clone(),
            self.uniswap_v2_factory,
            self.uniswap_v3_factory,
            self.weth,
            pools,
        );

        let res = deployer.call_raw().await?;
        let constructor_return = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
            DynSolType::Uint(8),
            DynSolType::Address,
            DynSolType::Uint(256),
        ])));

        let return_tokens = constructor_return.abi_decode_sequence(&res)?;
        if let Some(tokens) = return_tokens.as_array() {
            return tokens
                .iter()
                .map(|token| {
                    let pool_info = PoolInfoReturn::try_from(token)?;
                    Ok((pool_info.poolAddress, pool_info))
                })
                .collect::<Result<HashMap<_, _>>>();
        } else {
            Err(eyre!("Failed to decode return tokens"))
        }
    }
}

#[async_trait]
impl<const CHUNK_SIZE: usize, T, N, P> AMMFilter for ValueFilter<CHUNK_SIZE, T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>> {
        let pool_infos = amms
            .iter()
            .cloned()
            .map(|amm| {
                let pool_address = amm.address();
                // TODO FIXME: Need to update this when we have balancer/v3 support
                let pool_type = match amm {
                    AMM::UniswapV2Pool(_) => 0,
                    AMM::UniswapV3Pool(_) => 1,
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
                    .get(&pool_address)
                    .map_or(false, |pool_info_return| {
                        pool_info_return.wethValue > self.min_weth_threshold
                    })
            })
            .collect::<Vec<_>>();
        Ok(filtered_amms)
    }
}
