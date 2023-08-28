use ethers::{
    abi::{ParamType, Token},
    prelude::abigen,
    providers::Middleware,
    types::{Bytes, H160, U256},
};
use std::sync::Arc;

use crate::{
    amm::{factory::AutomatedMarketMakerFactory, factory::Factory, AutomatedMarketMaker, AMM},
    errors::AMMError,
};

use spinoff::{spinners, Color, Spinner};

pub const U256_10_POW_18: U256 = U256([1000000000000000000, 0, 0, 0]);
pub const U256_10_POW_6: U256 = U256([1000000, 0, 0, 0]);

#[allow(clippy::too_many_arguments)]
//Filter that removes AMMs with that contain less than a specified usd value
pub async fn filter_amms_below_usd_threshold<M: Middleware>(
    amms: Vec<AMM>,
    factories: &[Factory],
    usd_weth_pool: AMM,
    usd_value_in_pool_threshold: f64, // This is the threshold where we will filter out any pool with less value than this
    weth: H160,
    weth_value_in_token_to_weth_pool_threshold: U256, //This is the threshold where we will ignore any token price < threshold during batch calls
    step: usize,
    middleware: Arc<M>,
) -> Result<Vec<AMM>, AMMError<M>> {
    let mut spinner = Spinner::new(
        spinners::Dots,
        "Filtering AMMs below USD threshold...",
        Color::Blue,
    );

    let weth_usd_price = usd_weth_pool.calculate_price(weth)?;

    //Init a new vec to hold the filtered AMMs
    let mut filtered_amms = vec![];

    let weth_values_in_pools = get_weth_values_in_amms(
        &amms,
        factories,
        weth,
        weth_value_in_token_to_weth_pool_threshold,
        step,
        middleware,
    )
    .await?;

    for (i, weth_value) in weth_values_in_pools.iter().enumerate() {
        if (weth_value / U256_10_POW_18).as_u64() as f64 * weth_usd_price
            >= usd_value_in_pool_threshold
        {
            //TODO: using clone for now since we only do this once but find a better way in a future update
            filtered_amms.push(amms[i].clone());
        }
    }

    spinner.success("All AMMs filtered");
    Ok(filtered_amms)
}

//Filter that removes AMMs with that contain less than a specified weth value
//
pub async fn filter_amms_below_weth_threshold<M: Middleware>(
    amms: Vec<AMM>,
    factories: &[Factory],
    weth: H160,
    weth_value_in_pool_threshold: U256, // This is the threshold where we will filter out any pool with less value than this
    weth_value_in_token_to_weth_pool_threshold: U256, //This is the threshold where we will ignore any token price < threshold during batch calls
    step: usize,
    middleware: Arc<M>,
) -> Result<Vec<AMM>, AMMError<M>> {
    let mut spinner = Spinner::new(
        spinners::Dots,
        "Filtering AMMs below weth threshold...",
        Color::Blue,
    );

    let mut filtered_amms = vec![];

    let weth_values_in_pools = get_weth_values_in_amms(
        &amms,
        factories,
        weth,
        weth_value_in_token_to_weth_pool_threshold,
        step,
        middleware,
    )
    .await?;

    for (i, weth_value) in weth_values_in_pools.iter().enumerate() {
        if *weth_value >= weth_value_in_pool_threshold {
            //TODO: using clone for now since we only do this once but find a better way in a future update
            filtered_amms.push(amms[i].clone());
        }
    }

    spinner.success("All AMMs filtered");
    Ok(filtered_amms)
}

pub async fn get_weth_values_in_amms<M: Middleware>(
    amms: &[AMM],
    factories: &[Factory],
    weth: H160,
    weth_value_in_token_to_weth_pool_threshold: U256,
    step: usize,
    middleware: Arc<M>,
) -> Result<Vec<U256>, AMMError<M>> {
    //Init a new vec to hold the filtered pools
    let mut aggregate_weth_values_in_amms = vec![];

    let mut idx_from = 0;
    let mut idx_to = if step > amms.len() { amms.len() } else { step };

    for _ in (0..amms.len()).step_by(step) {
        let weth_values_in_amms = get_weth_value_in_amm_batch_request(
            &amms[idx_from..idx_to],
            factories,
            weth,
            weth_value_in_token_to_weth_pool_threshold,
            middleware.clone(),
        )
        .await?;

        //add weth values in pools to the aggregate array
        aggregate_weth_values_in_amms.extend(weth_values_in_amms);

        idx_from = idx_to;

        if idx_to + step > amms.len() {
            idx_to = amms.len() - 1
        } else {
            idx_to += step;
        }
    }

    Ok(aggregate_weth_values_in_amms)
}

abigen!(
    GetWethValueInAMMBatchRequest,
    "src/filters/batch_requests/GetWethValueInAMMBatchRequest.json";
);

async fn get_weth_value_in_amm_batch_request<M: Middleware>(
    amms: &[AMM],
    factories: &[Factory],
    weth: H160,
    weth_value_in_token_to_weth_pool_threshold: U256,
    middleware: Arc<M>,
) -> Result<Vec<U256>, AMMError<M>> {
    let mut weth_values_in_pools = vec![];

    let amms = amms
        .iter()
        .map(|a| Token::Address(a.address()))
        .collect::<Vec<Token>>();

    let factory_is_uni_v3 = factories
        .iter()
        .map(|d| match d {
            Factory::UniswapV2Factory(_) => Token::Bool(false),
            Factory::UniswapV3Factory(_) => Token::Bool(true),
        })
        .collect::<Vec<Token>>();

    let factories = factories
        .iter()
        .map(|f| Token::Address(f.address()))
        .collect::<Vec<Token>>();

    let constructor_args = Token::Tuple(vec![
        Token::Array(amms),
        Token::Array(factories),
        Token::Array(factory_is_uni_v3),
        Token::Address(weth),
        Token::Uint(weth_value_in_token_to_weth_pool_threshold),
    ]);

    let deployer = GetWethValueInAMMBatchRequest::deploy(middleware, constructor_args)?;
    let return_data: Bytes = deployer.call_raw().await?;

    let return_data_tokens = ethers::abi::decode(
        &[ParamType::Array(Box::new(ParamType::Uint(256)))],
        &return_data,
    )?;

    for token_array in return_data_tokens {
        if let Some(arr) = token_array.into_array() {
            for token in arr {
                if let Some(weth_value_in_pool) = token.into_uint() {
                    weth_values_in_pools.push(weth_value_in_pool);
                }
            }
        }
    }

    Ok(weth_values_in_pools)
}
