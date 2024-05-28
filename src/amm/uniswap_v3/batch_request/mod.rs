use std::{sync::Arc, vec};

use alloy::{
    dyn_abi::{DynSolType, DynSolValue},
    network::Network,
    providers::Provider,
    sol,
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

#[inline]
fn populate_pool_data_from_tokens(
    mut pool: UniswapV3Pool,
    tokens: &[DynSolValue],
) -> Option<UniswapV3Pool> {
    pool.token_a = tokens[0].as_address()?;
    pool.token_a_decimals = tokens[1].as_uint()?.0.to::<u8>();
    pool.token_b = tokens[2].as_address()?;
    pool.token_b_decimals = tokens[3].as_uint()?.0.to::<u8>();
    pool.liquidity = tokens[4].as_uint()?.0.to::<u128>();
    pool.sqrt_price = tokens[5].as_uint()?.0;
    pool.tick = tokens[6].as_int()?.0.as_i32();
    pool.tick_spacing = tokens[7].as_int()?.0.as_i32();
    pool.fee = tokens[8].as_uint()?.0.to::<u32>();

    Some(pool)
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

    let constructor_return = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
        DynSolType::Address,
        DynSolType::Uint(8),
        DynSolType::Address,
        DynSolType::Uint(8),
        DynSolType::Uint(128),
        DynSolType::Uint(160),
        DynSolType::Int(24),
        DynSolType::Int(24),
        DynSolType::Uint(24),
        DynSolType::Int(128),
    ])));
    let return_data_tokens = constructor_return.abi_decode_sequence(&res)?;

    if let Some(tokens_arr) = return_data_tokens.as_array() {
        for token in tokens_arr {
            let pool_data = token
                .as_tuple()
                .ok_or(AMMError::BatchRequestError(pool.address))?;

            *pool = populate_pool_data_from_tokens(pool.to_owned(), pool_data)
                .ok_or(AMMError::BatchRequestError(pool.address))?;
        }
    }

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
        tick_start,
        num_ticks,
        pool.tick_spacing,
    );
    let res = if let Some(block_number) = block_number {
        deployer.block(block_number.into()).call_raw().await?
    } else {
        deployer.call_raw().await?
    };

    let constructor_return = DynSolType::Tuple(vec![
        DynSolType::Array(Box::new(DynSolType::Tuple(vec![
            DynSolType::Bool,
            DynSolType::Int(24),
            DynSolType::Int(128),
        ]))),
        DynSolType::Uint(32),
    ]);
    let return_data_tokens = constructor_return.abi_decode_sequence(&res)?;

    let return_data_tuple = return_data_tokens
        .as_tuple()
        .ok_or(AMMError::BatchRequestError(pool.address))?;

    let tick_data_arr = return_data_tuple[0]
        .as_array()
        .ok_or(AMMError::BatchRequestError(pool.address))?;

    let mut tick_data = vec![];
    for tokens in tick_data_arr {
        if let Some(tick_data_tuple) = tokens.as_tuple() {
            let initialized = tick_data_tuple[0]
                .as_bool()
                .ok_or(AMMError::BatchRequestError(pool.address))?;

            let tick = tick_data_tuple[1]
                .as_int()
                .ok_or(AMMError::BatchRequestError(pool.address))?
                .0
                .as_i32();

            let liquidity_net = tick_data_tuple[2]
                .as_int()
                .ok_or(AMMError::BatchRequestError(pool.address))?
                .0
                .try_into()
                .map_err(|e| AMMError::EyreError(eyre::eyre!("{e}")))?;

            tick_data.push(UniswapV3TickData {
                initialized,
                tick,
                liquidity_net,
            });
        }
    }

    let block_number = return_data_tuple[1]
        .as_uint()
        .ok_or(AMMError::BatchRequestError(pool.address))?
        .0
        .to::<u64>();

    Ok((tick_data, block_number))
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

    let constructor_return = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
        DynSolType::Uint(128),
        DynSolType::Uint(160),
        DynSolType::Int(24),
        DynSolType::Int(128),
    ])));

    let return_data_tokens = constructor_return.abi_decode_sequence(&res)?;

    if let Some(tokens_arr) = return_data_tokens.as_array() {
        if tokens_arr.len() == 1 {
            if let Some(tokens_tup) = tokens_arr[0].as_tuple() {
                if tokens_tup[1]
                    .as_uint()
                    .ok_or(AMMError::BatchRequestError(pool.address))?
                    .0
                    .is_zero()
                {
                    return Err(AMMError::BatchRequestError(pool.address));
                } else {
                    pool.liquidity = tokens_tup[0]
                        .as_uint()
                        .ok_or(AMMError::BatchRequestError(pool.address))?
                        .0
                        .to::<u128>();
                    pool.sqrt_price = tokens_tup[1]
                        .as_uint()
                        .ok_or(AMMError::BatchRequestError(pool.address))?
                        .0;
                    pool.tick = tokens_tup[2]
                        .as_int()
                        .ok_or(AMMError::BatchRequestError(pool.address))?
                        .0
                        .as_i32();
                }
            }
        } else {
            return Err(AMMError::EyreError(eyre::eyre!(
                "Unexpected length of the batch static call"
            )));
        }
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

    let deployer = IGetUniswapV3PoolDataBatchRequest::deploy_builder(provider, target_addresses);
    let res = deployer.block(block_number.into()).call_raw().await?;

    let constructor_return = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
        DynSolType::Address,
        DynSolType::Uint(8),
        DynSolType::Address,
        DynSolType::Uint(8),
        DynSolType::Uint(128),
        DynSolType::Uint(160),
        DynSolType::Int(24),
        DynSolType::Int(24),
        DynSolType::Uint(24),
        DynSolType::Int(128),
    ])));
    let return_data_tokens = constructor_return.abi_decode_sequence(&res)?;

    let mut pool_idx = 0;
    if let Some(tokens_arr) = return_data_tokens.as_array() {
        for token in tokens_arr {
            if let Some(pool_data) = token.as_tuple() {
                if let Some(address) = pool_data[0].as_address() {
                    if !address.is_zero() {
                        // Update pool data
                        if let AMM::UniswapV3Pool(uniswap_v3_pool) = amms
                            .get_mut(pool_idx)
                            .expect("Pool idx should be in bounds")
                        {
                            if let Some(pool) = populate_pool_data_from_tokens(
                                uniswap_v3_pool.to_owned(),
                                pool_data,
                            ) {
                                tracing::trace!(?pool);
                                *uniswap_v3_pool = pool;
                            }
                        }
                    }
                }

                pool_idx += 1;
            }
        }
    }

    Ok(())
}
