use std::{sync::Arc, vec};

use alloy::{network::Network, providers::Provider, sol, transports::Transport};
use tracing::instrument;

use crate::{
    amm::{AutomatedMarketMaker, AMM},
    errors::AMMError,
};

use super::UniswapV3Pool;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetUniswapV3PoolDataBatchRequest,
    "src/amm/uniswap_v3/batch_request/GetUniswapV3PoolDataBatchRequestABI.json"
}

sol! {
    contract IGetUniswapV3PoolDataBatchReturn {
        function constructorReturn() external view returns ((address, uint8, address, uint8, uint128, uint160, int24, int24, uint24, int128)[] memory);
    }
}

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetUniswapV3TickDataBatchRequest,
    "src/amm/uniswap_v3/batch_request/GetUniswapV3TickDataBatchRequestABI.json"
}

sol! {
    contract IGetUniswapV3TickDataBatchReturn {
        function constructorReturn() external view returns ((bool, int24, int128)[] memory, uint32);
    }
}

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    ISyncUniswapV3PoolBatchRequest,
    "src/amm/uniswap_v3/batch_request/SyncUniswapV3PoolBatchRequestABI.json"
}

sol! {
    contract ISyncUniswapV3PoolBatchReturn {
        function constructorReturn() external view returns ((uint128, uint160, int24, int128)[] memory);
    }
}

pub async fn get_v3_pool_data_batch_request<T, N, P>(
    pool: &mut UniswapV3Pool,
    block_number: Option<u64>,
    provider: Arc<P>,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let deployer =
        IGetUniswapV3PoolDataBatchRequest::deploy_builder(provider.clone(), vec![pool.address])
            .with_sol_decoder::<IGetUniswapV3PoolDataBatchReturn::constructorReturnCall>();

    let IGetUniswapV3PoolDataBatchReturn::constructorReturnReturn { _0: pool_data } =
        if let Some(block_number) = block_number {
            deployer.block(block_number.into()).call().await?
        } else {
            deployer.call().await?
        };

    // make sure returned pool data len == 1
    let pool_data_len = pool_data.len();
    if pool_data_len != 1_usize {
        return Err(AMMError::EyreError(eyre::eyre!(
            "Unexpected return length, expected 1, returned {pool_data_len}"
        )));
    }

    // Update pool data
    pool.token_a = pool_data[0].0;
    pool.token_a_decimals = pool_data[0].1;
    pool.token_b = pool_data[0].2;
    pool.token_b_decimals = pool_data[0].3;
    pool.liquidity = pool_data[0].4;
    pool.sqrt_price = pool_data[0].5;
    pool.tick = pool_data[0].6;
    pool.tick_spacing = pool_data[0].7;
    pool.fee = pool_data[0].8;

    tracing::trace!(?pool);

    Ok(())
}

pub struct UniswapV3TickData {
    pub initialized: bool,
    pub tick: i32,
    pub liquidity_net: i128,
}

pub async fn get_uniswap_v3_tick_data_batch_request<T, N, P>(
    pool: &UniswapV3Pool,
    tick_start: i32,
    zero_for_one: bool,
    num_ticks: u16,
    block_number: Option<u64>,
    provider: Arc<P>,
) -> Result<(Vec<UniswapV3TickData>, u64), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let deployer = IGetUniswapV3TickDataBatchRequest::deploy_builder(
        provider.clone(),
        pool.address,
        zero_for_one,
        tick_start,
        num_ticks,
        pool.tick_spacing,
    )
    .with_sol_decoder::<IGetUniswapV3TickDataBatchReturn::constructorReturnCall>();

    let IGetUniswapV3TickDataBatchReturn::constructorReturnReturn {
        _0: tick_data_raw,
        _1: block_number,
    } = if let Some(block_number) = block_number {
        deployer.block(block_number.into()).call().await?
    } else {
        deployer.call().await?
    };

    let tick_data = tick_data_raw
        .into_iter()
        .map(|(initialized, tick, liquidity_net)| UniswapV3TickData {
            initialized,
            tick,
            liquidity_net,
        })
        .collect();

    Ok((tick_data, block_number as u64))
}

pub async fn sync_v3_pool_batch_request<T, N, P>(
    pool: &mut UniswapV3Pool,
    provider: Arc<P>,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let deployer = ISyncUniswapV3PoolBatchRequest::deploy_builder(provider.clone(), pool.address)
        .with_sol_decoder::<ISyncUniswapV3PoolBatchReturn::constructorReturnCall>();

    let ISyncUniswapV3PoolBatchReturn::constructorReturnReturn { _0: pool_data } =
        deployer.call().await?;

    // make sure returned pool data len == 1
    let pool_data_len = pool_data.len();
    if pool_data_len != 1_usize {
        return Err(AMMError::EyreError(eyre::eyre!(
            "Unexpected return length, expected 1, returned {pool_data_len}"
        )));
    }

    // Update pool data
    // If the sqrt_price is not zero, signaling that the pool data was populated
    if pool_data[0].1.is_zero() {
        return Err(AMMError::BatchRequestError(pool.address));
    } else {
        pool.liquidity = pool_data[0].0;
        pool.sqrt_price = pool_data[0].1;
        pool.tick = pool_data[0].2;
    }

    Ok(())
}

#[instrument(skip(provider) level = "debug")]
pub async fn get_amm_data_batch_request<T, N, P>(
    amms: &mut [AMM],
    block_number: u64,
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

    let deployer =
        IGetUniswapV3PoolDataBatchRequest::deploy_builder(provider.clone(), target_addresses)
            .with_sol_decoder::<IGetUniswapV3PoolDataBatchReturn::constructorReturnCall>();
    let IGetUniswapV3PoolDataBatchReturn::constructorReturnReturn { _0: amms_data } =
        deployer.block(block_number.into()).call().await?;

    let mut pool_idx = 0;
    for amm_data in amms_data {
        if !amm_data.0.is_zero() {
            if let AMM::UniswapV3Pool(uniswap_v3_pool) = amms
                .get_mut(pool_idx)
                .expect("Pool ifx should be in bounds")
            {
                uniswap_v3_pool.token_a = amm_data.0;
                uniswap_v3_pool.token_a_decimals = amm_data.1;
                uniswap_v3_pool.token_b = amm_data.2;
                uniswap_v3_pool.token_b_decimals = amm_data.3;
                uniswap_v3_pool.liquidity = amm_data.4;
                uniswap_v3_pool.sqrt_price = amm_data.5;
                uniswap_v3_pool.tick = amm_data.6;
                uniswap_v3_pool.tick_spacing = amm_data.7;
                uniswap_v3_pool.fee = amm_data.8;

                tracing::trace!(?uniswap_v3_pool);
            }

            pool_idx += 1;
        }
    }

    Ok(())
}
