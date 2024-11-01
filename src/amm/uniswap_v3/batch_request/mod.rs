use std::{sync::Arc, vec};

use alloy::{
    network::Network,
    primitives::{aliases::I24, Address, U256},
    providers::Provider,
    sol,
    sol_types::SolValue,
    transports::Transport,
};
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
    #[allow(missing_docs)]
    #[sol(rpc)]
    IGetUniswapV3TickDataBatchRequest,
    "src/amm/uniswap_v3/batch_request/GetUniswapV3TickDataBatchRequestABI.json"
}

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    ISyncUniswapV3PoolBatchRequest,
    "src/amm/uniswap_v3/batch_request/SyncUniswapV3PoolBatchRequestABI.json"
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
    let deployer = IGetUniswapV3PoolDataBatchRequest::deploy_builder(provider, vec![pool.address]);
    let res = if let Some(block_number) = block_number {
        deployer.block(block_number.into()).call_raw().await?
    } else {
        deployer.call_raw().await?
    };

    let data = <Vec<(Address, u16, Address, u16, u128, U256, i32, i32, u32, i128)> as SolValue>::abi_decode(&res, false)?;
    let (
        token_a,
        token_a_dec,
        token_b,
        token_b_dec,
        liquidity,
        sqrt_price,
        tick,
        tick_spacing,
        fee,
        _,
    ) = if !data.is_empty() {
        data[0]
    } else {
        return Err(AMMError::BatchRequestError(pool.address));
    };

    pool.token_a = token_a;
    pool.token_a_decimals = token_a_dec as u8;
    pool.token_b = token_b;
    pool.token_b_decimals = token_b_dec as u8;
    pool.liquidity = liquidity;
    pool.sqrt_price = sqrt_price;
    pool.tick = tick;
    pool.tick_spacing = tick_spacing;
    pool.fee = fee;

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
        provider,
        pool.address,
        zero_for_one,
        I24::unchecked_from(tick_start),
        num_ticks,
        I24::unchecked_from(pool.tick_spacing),
    );
    let res = if let Some(block_number) = block_number {
        deployer.block(block_number.into()).call_raw().await?
    } else {
        deployer.call_raw().await?
    };

    let (tick_data_vec, block_number) =
        <(Vec<(bool, i32, i128)>, u32) as SolValue>::abi_decode(&res, false)?;
    let mut tick_data = vec![];

    for (initialized, tick, liquidity_net) in tick_data_vec.into_iter() {
        tick_data.push(UniswapV3TickData {
            initialized,
            tick,
            liquidity_net,
        });
    }

    Ok((tick_data, block_number.into()))
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
    let deployer = ISyncUniswapV3PoolBatchRequest::deploy_builder(provider, vec![pool.address]);
    let res = deployer.call_raw().await?;

    let data = <Vec<(u128, U256, i32, i128)> as SolValue>::abi_decode(&res, false)?;

    let (liquidity, sqrt_price, tick, _) = if !data.is_empty() {
        if data.len() == 1 {
            data[0]
        } else {
            return Err(AMMError::EyreError(eyre::eyre!(
                "Unexpected length of the batch static call"
            )));
        }
    } else {
        return Err(AMMError::BatchRequestError(pool.address));
    };

    pool.liquidity = liquidity;
    pool.sqrt_price = sqrt_price;
    pool.tick = tick;

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

    let deployer = IGetUniswapV3PoolDataBatchRequest::deploy_builder(provider, target_addresses);
    let res = deployer.block(block_number.into()).call_raw().await?;

    let pools = <Vec<(Address, u16, Address, u16, u128, U256, i32, i32, u32, i128)> as SolValue>::abi_decode(&res, false)?;

    for (
        pool_idx,
        (
            token_a,
            token_a_dec,
            token_b,
            token_b_dec,
            liquidity,
            sqrt_price,
            tick,
            tick_spacing,
            fee,
            _,
        ),
    ) in pools.into_iter().enumerate()
    {
        // If the pool token A is not zero, signaling that the pool data was polulated
        if !token_a.is_zero() {
            if let AMM::UniswapV3Pool(pool) = amms
                .get_mut(pool_idx)
                .expect("Pool idx should be in bounds")
            {
                pool.token_a = token_a;
                pool.token_a_decimals = token_a_dec as u8;
                pool.token_b = token_b;
                pool.token_b_decimals = token_b_dec as u8;
                pool.liquidity = liquidity;
                pool.sqrt_price = sqrt_price;
                pool.tick = tick;
                pool.tick_spacing = tick_spacing;
                pool.fee = fee;

                tracing::trace!(?pool);
            }
        }
    }

    Ok(())
}
