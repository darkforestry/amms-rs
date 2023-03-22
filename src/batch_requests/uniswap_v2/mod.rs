use ethers::{
    abi::{ParamType, Token},
    prelude::abigen,
    providers::Middleware,
    types::{Bytes, H160, U256},
};
use std::sync::Arc;

use crate::{
    errors::CFMMError,
    pool::{Pool, UniswapV2Pool},
};

abigen!(
    GetUniswapV2PairsBatchRequest,
    "src/batch_requests/uniswap_v2/GetUniswapV2PairsBatchRequest.json";
    GetUniswapV2PoolDataBatchRequest,
    "src/batch_requests/uniswap_v2/GetUniswapV2PoolDataBatchRequest.json";
);

pub async fn get_pairs_batch_request<M: Middleware>(
    factory: H160,
    from: U256,
    step: U256,
    middleware: Arc<M>,
) -> Result<Vec<H160>, CFMMError<M>> {
    let mut pairs = vec![];

    let constructor_args = Token::Tuple(vec![
        Token::Uint(from),
        Token::Uint(step),
        Token::Address(factory),
    ]);

    let deployer = GetUniswapV2PairsBatchRequest::deploy(middleware, constructor_args).unwrap();
    let return_data: Bytes = deployer.call_raw().await?;

    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Address))],
        &return_data,
    )?;

    for token_array in return_data_tokens {
        if let Some(arr) = token_array.into_array() {
            for token in arr {
                if let Some(addr) = token.into_address() {
                    if !addr.is_zero() {
                        pairs.push(addr);
                    }
                }
            }
        }
    }

    Ok(pairs)
}

pub async fn get_pool_data_batch_request<M: Middleware>(
    pools: &mut [Pool],
    middleware: Arc<M>,
) -> Result<(), CFMMError<M>> {
    let mut target_addresses = vec![];
    for pool in pools.iter() {
        target_addresses.push(Token::Address(pool.address()));
    }

    let constructor_args = Token::Tuple(vec![Token::Array(target_addresses)]);

    let deployer =
        GetUniswapV2PoolDataBatchRequest::deploy(middleware.clone(), constructor_args).unwrap();

    let return_data: Bytes = deployer.call_raw().await?;
    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Tuple(vec![
            ParamType::Address,   // token a
            ParamType::Uint(8),   // token a decimals
            ParamType::Address,   // token b
            ParamType::Uint(8),   // token b decimals
            ParamType::Uint(112), // reserve 0
            ParamType::Uint(112), // reserve 1
        ])))],
        &return_data,
    )?;

    let mut pool_idx = 0;

    for tokens in return_data_tokens {
        if let Some(tokens_arr) = tokens.into_array() {
            for tup in tokens_arr {
                if let Some(pool_data) = tup.into_tuple() {
                    //If the pool token A is not zero, signaling that the pool data was populated
                    if !pool_data[0].to_owned().into_address().unwrap().is_zero() {
                        //Update the pool data
                        if let Pool::UniswapV2(uniswap_v2_pool) = pools.get_mut(pool_idx).unwrap() {
                            uniswap_v2_pool.token_a =
                                pool_data[0].to_owned().into_address().unwrap();
                            uniswap_v2_pool.token_a_decimals =
                                pool_data[1].to_owned().into_uint().unwrap().as_u32() as u8;
                            uniswap_v2_pool.token_b =
                                pool_data[2].to_owned().into_address().unwrap();
                            uniswap_v2_pool.token_b_decimals =
                                pool_data[3].to_owned().into_uint().unwrap().as_u32() as u8;
                            uniswap_v2_pool.reserve_0 =
                                pool_data[4].to_owned().into_uint().unwrap().as_u128();
                            uniswap_v2_pool.reserve_1 =
                                pool_data[5].to_owned().into_uint().unwrap().as_u128();

                            uniswap_v2_pool.fee = 300;
                        }
                    }
                    pool_idx += 1;
                }
            }
        }
    }

    Ok(())
}

pub async fn get_v2_pool_data_batch_request<M: Middleware>(
    pool: &mut UniswapV2Pool,
    middleware: Arc<M>,
) -> Result<(), CFMMError<M>> {
    let constructor_args = Token::Tuple(vec![Token::Array(vec![Token::Address(pool.address())])]);

    let deployer =
        GetUniswapV2PoolDataBatchRequest::deploy(middleware.clone(), constructor_args).unwrap();

    let return_data: Bytes = deployer.call_raw().await?;
    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Tuple(vec![
            ParamType::Address,   // token a
            ParamType::Uint(8),   // token a decimals
            ParamType::Address,   // token b
            ParamType::Uint(8),   // token b decimals
            ParamType::Uint(112), // reserve 0
            ParamType::Uint(112), // reserve 1
        ])))],
        &return_data,
    )?;

    for tokens in return_data_tokens {
        if let Some(tokens_arr) = tokens.into_array() {
            for tup in tokens_arr {
                if let Some(pool_data) = tup.into_tuple() {
                    //If the pool token A is not zero, signaling that the pool data was populated
                    if !pool_data[0].to_owned().into_address().unwrap().is_zero() {
                        //Update the pool data
                        pool.token_a = pool_data[0].to_owned().into_address().unwrap();
                        pool.token_a_decimals =
                            pool_data[1].to_owned().into_uint().unwrap().as_u32() as u8;
                        pool.token_b = pool_data[2].to_owned().into_address().unwrap();
                        pool.token_b_decimals =
                            pool_data[3].to_owned().into_uint().unwrap().as_u32() as u8;
                        pool.reserve_0 = pool_data[4].to_owned().into_uint().unwrap().as_u128();
                        pool.reserve_1 = pool_data[5].to_owned().into_uint().unwrap().as_u128();

                        pool.fee = 300;
                    }
                }
            }
        }
    }

    Ok(())
}
