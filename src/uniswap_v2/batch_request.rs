use ethers::{
    abi::{ParamType, Token},
    providers::Middleware,
    types::Bytes,
};
use std::sync::Arc;

use crate::{errors::DAMMError, interfaces};

use super::UniswapV2Pool;

pub async fn get_v2_pool_data_batch_request<M: Middleware>(
    pool: &mut UniswapV2Pool,
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    let constructor_args = Token::Tuple(vec![Token::Array(vec![Token::Address(pool.address)])]);

    let deployer =
        interfaces::IGetUniswapV2PoolDataBatchRequest::deploy(middleware.clone(), constructor_args)
            .unwrap();

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
