use std::{sync::Arc, vec};

use ethers::{
    abi::{ParamType, Token},
    providers::Middleware,
    types::{Bytes, I256},
};

use crate::{
    amm::{AutomatedMarketMaker, AMM},
    errors::DAMMError,
};

use super::IZiSwapPool;

use ethers::prelude::abigen;

abigen!(
    IGetiZiPoolDataBatchRequest,
    "src/amm/izumi/batch_request/GetiZiPoolDataBatchRequest.json";
    ISynciZiPoolDataBatchRequest,
    "src/amm/izumi/batch_request/SynciZiPoolDataBatchRequest.json";

);

pub async fn get_izi_pool_data_batch_request<M: Middleware>(
    pool: &mut IZiSwapPool,
    block_number: Option<u64>,
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    let constructor_args = Token::Tuple(vec![Token::Array(vec![Token::Address(pool.address)])]);

    let deployer =
        IGetiZiPoolDataBatchRequest::deploy(middleware.clone(), constructor_args).unwrap();

    let return_data: Bytes = if let Some(block_number) = block_number {
        deployer.block(block_number).call_raw().await?
    } else {
        deployer.call_raw().await?
    };

    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Tuple(vec![
            ParamType::Address,   // token a
            ParamType::Uint(8),   // token a decimals
            ParamType::Address,   // token b
            ParamType::Uint(8),   // token b decimals
            ParamType::Uint(128), // liquidity
            ParamType::Uint(160), // sqrtPrice
            ParamType::Uint(128), // liquidityA
            ParamType::Uint(128), // liquidityB
            ParamType::Int(24),   // currentPoint
            ParamType::Int(24),   // pointDelta
            ParamType::Uint(24),  //fee
        ])))],
        &return_data,
    )?;

    //Update pool data
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

                        pool.liquidity = pool_data[4].to_owned().into_uint().unwrap().as_u128();

                        pool.sqrt_price = pool_data[5].to_owned().into_uint().unwrap();

                        pool.liquidity_x =
                            I256::from_raw(pool_data[6].to_owned().into_uint().unwrap()).as_u128();

                        pool.liquidity_y =
                            I256::from_raw(pool_data[7].to_owned().into_uint().unwrap()).as_u128();

                        pool.current_point =
                            I256::from_raw(pool_data[8].to_owned().into_int().unwrap()).as_i32();
                        pool.point_delta =
                            I256::from_raw(pool_data[9].to_owned().into_int().unwrap()).as_i32();
                        pool.fee = pool_data[10].to_owned().into_uint().unwrap().as_u64() as u32;
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn sync_izi_pool_batch_request<M: Middleware>(
    pool: &mut IZiSwapPool,
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    let constructor_args = Token::Tuple(vec![Token::Address(pool.address)]);

    let deployer =
        ISynciZiPoolDataBatchRequest::deploy(middleware.clone(), constructor_args).unwrap();

    let return_data: Bytes = deployer.call_raw().await?;
    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Tuple(vec![
            ParamType::Uint(128), // liquidity
            ParamType::Uint(160), // sqrtPrice
            ParamType::Uint(128), // la
            ParamType::Uint(128), // lb
            ParamType::Int(24),   // currentPoint
        ])],
        &return_data,
    )?;

    for tokens in return_data_tokens {
        if let Some(pool_data) = tokens.into_tuple() {
            //If the sqrt_price is not zero, signaling that the pool data was populated
            if !pool_data[1].to_owned().into_uint().unwrap().is_zero() {
                //Update the pool data
                pool.liquidity = pool_data[0].to_owned().into_uint().unwrap().as_u128();
                pool.sqrt_price = pool_data[1].to_owned().into_uint().unwrap();
                pool.liquidity_x = pool_data[2].to_owned().into_uint().unwrap().as_u128();
                pool.liquidity_y = pool_data[3].to_owned().into_uint().unwrap().as_u128();
                pool.current_point =
                    I256::from_raw(pool_data[4].to_owned().into_int().unwrap()).as_i32();
            } else {
                return Err(DAMMError::SyncError(pool.address));
            }
        }
    }

    Ok(())
}

pub async fn get_amm_data_batch_request<M: Middleware>(
    amms: &mut [AMM],
    block_number: u64,
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    let mut target_addresses = vec![];

    for amm in amms.iter() {
        target_addresses.push(Token::Address(amm.address()));
    }

    let constructor_args = Token::Tuple(vec![Token::Array(target_addresses)]);
    let deployer =
        IGetiZiPoolDataBatchRequest::deploy(middleware.clone(), constructor_args).unwrap();

    let return_data: Bytes = deployer.block(block_number).call_raw().await?;

    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Tuple(vec![
            ParamType::Address,   // token a
            ParamType::Uint(8),   // token a decimals
            ParamType::Address,   // token b
            ParamType::Uint(8),   // token b decimals
            ParamType::Uint(128), // liquidity
            ParamType::Uint(160), // sqrtPrice
            ParamType::Uint(128), // liquidityA
            ParamType::Uint(128), // liquidityB
            ParamType::Int(24),   // currentPoint
            ParamType::Int(24),   // pointDelta
            ParamType::Uint(24),  //fee
        ])))],
        &return_data,
    )?;

    let mut pool_idx = 0;

    //Update pool data
    for tokens in return_data_tokens {
        if let Some(tokens_arr) = tokens.into_array() {
            for tup in tokens_arr {
                if let Some(pool_data) = tup.into_tuple() {
                    //If the pool token A is not zero, signaling that the pool data was populated
                    if !pool_data[0].to_owned().into_address().unwrap().is_zero() {
                        //Update the pool data
                        if let AMM::IZiSwapPool(izi_pool) = amms.get_mut(pool_idx).unwrap() {
                            izi_pool.token_a = pool_data[0].to_owned().into_address().unwrap();

                            izi_pool.token_a_decimals =
                                pool_data[1].to_owned().into_uint().unwrap().as_u32() as u8;

                            izi_pool.token_b = pool_data[2].to_owned().into_address().unwrap();

                            izi_pool.token_b_decimals =
                                pool_data[3].to_owned().into_uint().unwrap().as_u32() as u8;

                            izi_pool.liquidity =
                                pool_data[4].to_owned().into_uint().unwrap().as_u128();

                            izi_pool.sqrt_price = pool_data[5].to_owned().into_uint().unwrap();

                            izi_pool.liquidity_x =
                                pool_data[6].to_owned().into_uint().unwrap().as_u128();

                            izi_pool.liquidity_y =
                                pool_data[7].to_owned().into_uint().unwrap().as_u128();
                            izi_pool.current_point =
                                I256::from_raw(pool_data[8].to_owned().into_int().unwrap())
                                    .as_i32();
                            izi_pool.point_delta =
                                I256::from_raw(pool_data[8].to_owned().into_int().unwrap())
                                    .as_i32();

                            izi_pool.fee =
                                pool_data[9].to_owned().into_uint().unwrap().as_u64() as u32;
                        }
                    }
                    pool_idx += 1;
                }
            }
        }
    }
    Ok(())
}
