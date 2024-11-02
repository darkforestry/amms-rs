use alloy::{
    network::Network,
    primitives::{Address, U256},
    providers::Provider,
    sol,
    sol_types::SolValue,
    transports::Transport,
};

use crate::{
    amm::{AutomatedMarketMaker, AMM},
    errors::AMMError,
};

use super::BalancerV2Pool;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetBalancerV2PoolDataBatchRequest,
    "src/amm/balancer_v2/batch_request/GetBalancerV2PoolDataBatchRequest.json"
}

pub async fn get_balancer_v2_pool_data_batch_request<T, N, P>(
    pool: &mut BalancerV2Pool,
    block_number: Option<u64>,
    provider: P,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Clone,
{
    let deployer = IGetBalancerV2PoolDataBatchRequest::deploy_builder(provider, vec![pool.address]);
    let res = if let Some(block_number) = block_number {
        deployer.block(block_number.into()).call_raw().await?
    } else {
        deployer.call_raw().await?
    };

    let mut data =
        <Vec<(Vec<Address>, Vec<u16>, Vec<U256>, Vec<U256>, u32)> as SolValue>::abi_decode(
            &res, false,
        )?;
    let (tokens, decimals, liquidity, weights, fee) = if !data.is_empty() {
        data.remove(0)
    } else {
        return Err(AMMError::BatchRequestError(pool.address));
    };

    pool.tokens = tokens;
    pool.decimals = decimals.into_iter().map(|d| d as u8).collect();
    pool.liquidity = liquidity;
    pool.weights = weights;
    pool.fee = fee;

    tracing::trace!(?pool);

    Ok(())
}

pub async fn get_amm_data_batch_request<T, N, P>(
    amms: &mut [AMM],
    provider: P,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Clone,
{
    let deployer = IGetBalancerV2PoolDataBatchRequest::deploy_builder(
        provider,
        amms.iter().map(|amm| amm.address()).collect(),
    );
    let res = deployer.call_raw().await?;

    let pools = <Vec<(Vec<Address>, Vec<u16>, Vec<U256>, Vec<U256>, u32)> as SolValue>::abi_decode(
        &res, false,
    )?;

    for (pool_idx, (tokens, decimals, liquidity, weights, fee)) in pools.into_iter().enumerate() {
        if let AMM::BalancerV2Pool(pool) = amms
            .get_mut(pool_idx)
            .expect("Pool idx should be in bounds")
        {
            pool.tokens = tokens;
            pool.decimals = decimals.into_iter().map(|d| d as u8).collect();
            pool.liquidity = liquidity;
            pool.weights = weights;
            pool.fee = fee;

            tracing::trace!(?pool);
        }
    }

    Ok(())
}
