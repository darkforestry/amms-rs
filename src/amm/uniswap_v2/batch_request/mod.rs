use alloy::{
    network::Network,
    primitives::{Address, U256},
    providers::Provider,
    sol,
    sol_types::SolValue,
    transports::Transport,
};
use std::sync::Arc;

use crate::{
    amm::{AutomatedMarketMaker, AMM},
    errors::AMMError,
};

use super::UniswapV2Pool;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetUniswapV2PairsBatchRequest,
    "src/amm/uniswap_v2/batch_request/GetUniswapV2PairsBatchRequestABI.json"
}

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetUniswapV2PoolDataBatchRequest,
    "src/amm/uniswap_v2/batch_request/GetUniswapV2PoolDataBatchRequestABI.json"
}

pub async fn get_pairs_batch_request<T, N, P>(
    factory: Address,
    from: U256,
    step: U256,
    provider: Arc<P>,
) -> Result<Vec<Address>, AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let deployer = IGetUniswapV2PairsBatchRequest::deploy_builder(provider, from, step, factory);
    let res = deployer.call_raw().await?;
    Ok(<Vec<Address> as SolValue>::abi_decode(&res, false)?)
}

pub async fn get_amm_data_batch_request<T, N, P>(
    amms: &mut [AMM],
    provider: Arc<P>,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let mut target_addresses = vec![];
    for amm in amms.iter() {
        target_addresses.push(amm.address());
    }

    let deployer = IGetUniswapV2PoolDataBatchRequest::deploy_builder(provider, target_addresses);
    let res = deployer.call().await?;

    let pools =
        <Vec<(Address, u16, Address, u16, u128, u128)> as SolValue>::abi_decode(&res, false)?;

    for (pool_idx, (token_a, token_a_dec, token_b, token_b_dec, reserve_0, reserve_1)) in
        pools.into_iter().enumerate()
    {
        // If the pool token A is not zero, signaling that the pool data was polulated
        if !token_a.is_zero() {
            if let AMM::UniswapV2Pool(pool) = amms
                .get_mut(pool_idx)
                .expect("Pool idx should be in bounds")
            {
                pool.token_a = token_a;
                pool.token_a_decimals = token_a_dec as u8;
                pool.token_b = token_b;
                pool.token_b_decimals = token_b_dec as u8;
                pool.reserve_0 = reserve_0;
                pool.reserve_1 = reserve_1;

                tracing::trace!(?pool);
            }
        }
    }

    Ok(())
}

pub async fn get_v2_pool_data_batch_request<T, N, P>(
    pool: &mut UniswapV2Pool,
    provider: Arc<P>,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let deployer = IGetUniswapV2PoolDataBatchRequest::deploy_builder(provider, vec![pool.address]);
    let res = deployer.call_raw().await?;

    let data =
        <Vec<(Address, u16, Address, u16, u128, u128)> as SolValue>::abi_decode(&res, false)?;
    let (token_a, token_a_dec, token_b, token_b_dec, reserve_0, reserve_1) = if !data.is_empty() {
        data[0]
    } else {
        return Err(AMMError::BatchRequestError(pool.address));
    };

    pool.token_a = token_a;
    pool.token_a_decimals = token_a_dec as u8;
    pool.token_b = token_b;
    pool.token_b_decimals = token_b_dec as u8;
    pool.reserve_0 = reserve_0;
    pool.reserve_1 = reserve_1;

    tracing::trace!(?pool);

    Ok(())
}
