use std::sync::Arc;

use alloy::{
    dyn_abi::{DynSolType, DynSolValue},
    network::Network,
    providers::Provider,
    sol,
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

#[inline]
fn populate_pool_data_from_tokens(pool: &mut BalancerV2Pool, tokens: &[DynSolValue]) {
    // TODO: Add error handling
    pool.tokens = tokens[0]
        .as_array()
        .expect("Expected array")
        .iter()
        .map(|t| t.as_address().expect("Expected address"))
        .collect();
    pool.decimals = tokens[1]
        .as_array()
        .expect("Expected array")
        .iter()
        .map(|t| t.as_uint().expect("Expected uint").0.to::<u8>())
        .collect();
    pool.liquidity = tokens[2]
        .as_array()
        .expect("Expected array")
        .iter()
        .map(|t| t.as_uint().expect("Expected uint").0)
        .collect();
    pool.weights = tokens[3]
        .as_array()
        .expect("Expected array")
        .iter()
        .map(|t| t.as_uint().expect("Expected uint").0)
        .collect();
    pool.fee = tokens[4].as_uint().expect("Expected uint").0.to::<u32>();
}

pub async fn get_balancer_v2_pool_data_batch_request<T, N, P>(
    pool: &mut BalancerV2Pool,
    block_number: Option<u64>,
    provider: Arc<P>,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let deployer = IGetBalancerV2PoolDataBatchRequest::deploy_builder(provider, vec![pool.address]);
    let res = if let Some(block_number) = block_number {
        deployer.block(block_number.into()).call_raw().await?
    } else {
        deployer.call_raw().await?
    };

    let constructor_return = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
        DynSolType::Array(Box::new(DynSolType::Address)),
        DynSolType::Array(Box::new(DynSolType::Uint(8))),
        DynSolType::Array(Box::new(DynSolType::Uint(256))),
        DynSolType::Array(Box::new(DynSolType::Uint(256))),
        DynSolType::Uint(32),
    ])));

    let return_data_tokens = constructor_return.abi_decode_sequence(&res)?;

    if let Some(tokens_arr) = return_data_tokens.as_array() {
        for token in tokens_arr {
            let pool_data = token
                .as_tuple()
                .ok_or(AMMError::BatchRequestError(pool.address))?;

            populate_pool_data_from_tokens(pool, pool_data);
        }
    }

    Ok(())
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
    let deployer = IGetBalancerV2PoolDataBatchRequest::deploy_builder(
        provider,
        amms.iter().map(|amm| amm.address()).collect(),
    );
    let res = deployer.call_raw().await?;

    let constructor_return = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
        DynSolType::Array(Box::new(DynSolType::Address)),
        DynSolType::Array(Box::new(DynSolType::Uint(8))),
        DynSolType::Array(Box::new(DynSolType::Uint(256))),
        DynSolType::Uint(32),
    ])));

    let return_data_tokens = constructor_return.abi_decode_sequence(&res)?;

    if let Some(tokens_arr) = return_data_tokens.as_array() {
        for (i, token) in tokens_arr.into_iter().enumerate() {
            let pool_data = token
                .as_tuple()
                .ok_or(AMMError::BatchRequestError(amms[i].address()))?;
            if let AMM::BalancerV2Pool(pool) = &mut amms[i] {
                populate_pool_data_from_tokens(pool, pool_data);
            }
        }
    }
    Ok(())
}
