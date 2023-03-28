use core::panic;
use std::{
    fs::read_to_string,
    panic::resume_unwind,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use ethers::{
    providers::Middleware,
    types::{BlockNumber, H160, U256},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Serialize, Deserialize};
use serde_json::{Map, Value};
use tokio::task::JoinHandle;

use crate::{
    amm::{AMM, factory::{Factory, AutomatedMarketMakerFactory}},
    errors::DAMMError,
    sync,
};

#[derive(Serialize, Deserialize)]
pub struct Checkpoint{
    pub timestamp: usize,
    pub block_number: u64,    
    pub factories: Vec<Factory>,
    pub amms: Vec<AMM>,
}

//Get all pairs from last synced block and sync reserve values for each Dex in the `dexes` vec.
pub async fn sync_amms_from_checkpoint<M: 'static + Middleware>(
    path_to_checkpoint: &str,
    step: usize,
    requests_per_second_limit: usize,
    middleware: Arc<M>,
) -> Result<(Vec<Factory>, Vec<AMM>), DAMMError<M>> {
    let current_block = middleware
        .get_block_number()
        .await
        .map_err(DAMMError::MiddlewareError)?;

      let checkpoint: Checkpoint =   serde_json::from_str(
            read_to_string(path_to_checkpoint)
                .expect("Error when reading in checkpoint json")
                .as_str(),
        )
        .expect("Error when converting checkpoint file contents to serde_json::Value");


    //Sort all of the pools from the checkpoint into uniswap_v2_pools and uniswap_v3_pools pools so we can sync them concurrently
    let (uniswap_v2_pools, uniswap_v3_pools) = sort_amms(checkpoint.amms);

    let mut aggregated_amms = vec![];
    let mut handles = vec![];

    //Sync all uniswap v2 pools from checkpoint
    if !uniswap_v2_pools.is_empty() {
        handles.push(
            batch_sync_pools_from_checkpoint(
                uniswap_v2_pools,
                DexVariant::UniswapV2,
                multi_progress_bar.add(ProgressBar::new(0)),
                request_throttle.clone(),
                middleware.clone(),
            )
            .await,
        );
    }

    //Sync all uniswap v3 pools from checkpoint
    if !uniswap_v3_pools.is_empty() {
        handles.push(
            batch_sync_pools_from_checkpoint(
                uniswap_v3_pools,
                DexVariant::UniswapV3,
                multi_progress_bar.add(ProgressBar::new(0)),
                request_throttle.clone(),
                middleware.clone(),
            )
            .await,
        );
    }

    //Sync all pools from the since synced block
    handles.extend(
        get_new_pools_from_range(
            dexes.clone(),
            checkpoint_block_number,
            current_block.into(),
            step,
            request_throttle,
            multi_progress_bar,
            middleware.clone(),
        )
        .await,
    );

    for handle in handles {
        match handle.await {
            Ok(sync_result) => aggregated_pools.extend(sync_result?),
            Err(err) => {
                {
                    if err.is_panic() {
                        // Resume the panic on the main task
                        resume_unwind(err.into_panic());
                    }
                }
            }
        }
    }

    //update the sync checkpoint
    construct_checkpoint(
        dexes.clone(),
        &aggregated_pools,
        current_block.as_u64(),
        path_to_checkpoint,
    );

    Ok((dexes, aggregated_pools))
}




pub async fn batch_sync_amms_from_checkpoint<M: 'static + Middleware>(
    mut pools: Vec<AMM>,
    middleware: Arc<M>,
) -> JoinHandle<Result<Vec<AMM>, DAMMError<M>>> {



    //Spawn a new thread to get all pools and sync data for each dex
    tokio::spawn(async move {
        //Get all pool data via batched calls
        fac.get_all_pool_data(&mut pools, request_throttle, progress_bar, middleware)
            .await?;

        //Clean empty pools
        pools = sync::remove_empty_pools(pools);

        Ok::<_, CFMMError<M>>(pools)
    })
}

pub fn sort_amms(amms: Vec<AMM>) -> (Vec<AMM>, Vec<AMM>) {
    let mut uniswap_v2_pools = vec![];
    let mut uniswap_v3_pools = vec![];

    for amm in amms {
        match amm {
            AMM::UniswapV2Pool(_) => uniswap_v2_pools.push(amm),
            AMM::UniswapV3Pool(_) => uniswap_v3_pools.push(amm),
        }
    }

    (uniswap_v2_pools, uniswap_v3_pools)
}

pub async fn get_new_pools_from_range<M: 'static + Middleware>(
    factories: Vec<Factory>,
    from_block: BlockNumber,
    to_block: BlockNumber,
    step: usize,
    middleware: Arc<M>,
) -> Vec<JoinHandle<Result<Vec<AMM>, DAMMError<M>>>> {
    //Create the filter with all the pair created events
    //Aggregate the populated pools from each thread
    let mut handles = vec![];

    for factory in factories {
        let middleware = middleware.clone();

        //Spawn a new thread to get all pools and sync data for each dex
        handles.push(tokio::spawn(async move {

            
            let mut pools = factory
                .get_all_pools_from_logs(
                    from_block,
                    to_block,
                    step,
                    middleware.clone(),
                )
                .await?;


                factory.populate_amm_data(
                &mut pools,
                middleware.clone(),
            )
            .await?;

            //Clean empty pools
            pools = sync::remove_empty_amms(pools);

            Ok::<_, DAMMError<M>>(pools)
        }));
    }

    handles
}




pub fn deconstruct_factory_from_checkpoint(dex_map: &Map<String, Value>) -> Factory {

    let block_number = dex_map
        .get("block_number")
        .expect("Checkpoint formatted incorrectly, could not get dex latest_synced_block.")
        .as_u64()
        .expect("Could not convert latest_synced_block to u64");

    let factory_address = H160::from_str(
        dex_map
            .get("address")
            .expect("Checkpoint formatted incorrectly, could not get dex factory_address.")
            .as_str()
            .expect("Could not convert factory_address to str"),
    )
    .expect("Could not convert checkpoint factory_address to H160.");

    let fee = dex_map
        .get("fee")
        .map(|fee| fee.as_u64().expect("Could not convert fee to u64"));

    Dex::new(factory_address, dex_variant, block_number, fee)

    match dex_map
        .get("factory_variant")
        .expect("Checkpoint formatted incorrectly, could not get factory_variant.")
        .as_str()
        .expect("Could not convert dex variant to string")
        .to_lowercase()
        .as_str()
    {
        "UniswapV2Factory" => {Factory::UniswapV2Factory()},
        "UniswapV3Factory" => Factory::UniswapV3Factory(),
        other => {
            panic!("Unrecognized factory variant in checkpoint: {:?}", other)
        }
    };
}


pub fn construct_checkpoint(
    factories: Vec<Factory>,
    amms: &Vec<AMM>,
    latest_block: u64,
    checkpoint_path: &str,
) {
    let mut checkpoint = Map::new();

    //Insert checkpoint_timestamp
    let checkpoint_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f32() as u32;

    checkpoint.insert(
        String::from("checkpoint_timestamp"),
        checkpoint_timestamp.into(),
    );

    checkpoint.insert(String::from("block_number"), latest_block.into());

    //Add factories to checkpoint
    let mut factories_array: Vec<Value> = vec![];
    for factory in factories {
        let mut factory_map = Map::new();

        factory_map.insert(
            String::from("address"),
            format!("{:?}", factory.address()).into(),
        );

        factory_map.insert(String::from("block_number"), latest_block.into());

        match factory {
            Factory::UniswapV2Factory(uniswap_v2_factory) => {
                factory_map.insert(
                    String::from("factory_variant"),
                    String::from("UniswapV2Factory").into(),
                );

                factory_map.insert(
                    String::from("fee"),
                    format!("{:?}", uniswap_v2_factory.fee).into(),
                );
            }

            Dex::UniswapV3(_) => {
                factory_map.insert(
                    String::from("factory_variant"),
                    String::from("UniswapV3Factory").into(),
                );
            }
        }

        factories_array.push(Value::Object(factory_map));
    }

    checkpoint.insert(String::from("factories"), factories_array.into());

    //Insert amms into checkpoint
    let mut amms_array: Vec<Value> = vec![];
    for amm in amms {
        let mut amm_map = Map::new();

        match amm {
            Pool::UniswapV2(uniswap_v2_pool) => {
                amm_map.insert(
                    String::from("amm_variant"),
                    String::from("UniswapV2").into(),
                );

                amm_map.insert(
                    String::from("address"),
                    format!("{:?}", uniswap_v2_pool.address).into(),
                );

                amm_map.insert(
                    String::from("token_a"),
                    format!("{:?}", uniswap_v2_pool.token_a).into(),
                );

                amm_map.insert(
                    String::from("token_a_decimals"),
                    uniswap_v2_pool.token_a_decimals.into(),
                );

                amm_map.insert(
                    String::from("token_b"),
                    format!("{:?}", uniswap_v2_pool.token_b).into(),
                );

                amm_map.insert(
                    String::from("token_b_decimals"),
                    uniswap_v2_pool.token_b_decimals.into(),
                );

                amm_map.insert(String::from("fee"), uniswap_v2_pool.fee.into());

                amms_array.push(amm_map.into());
            }

            Pool::UniswapV3(uniswap_v3_pool) => {
                amm_map.insert(
                    String::from("dex_variant"),
                    String::from("UniswapV3").into(),
                );

                amm_map.insert(
                    String::from("address"),
                    format!("{:?}", uniswap_v3_pool.address).into(),
                );

                amm_map.insert(
                    String::from("token_a"),
                    format!("{:?}", uniswap_v3_pool.token_a).into(),
                );

                amm_map.insert(
                    String::from("token_a_decimals"),
                    uniswap_v3_pool.token_a_decimals.into(),
                );

                amm_map.insert(
                    String::from("token_b"),
                    format!("{:?}", uniswap_v3_pool.token_b).into(),
                );

                amm_map.insert(
                    String::from("token_b_decimals"),
                    uniswap_v3_pool.token_b_decimals.into(),
                );

                amm_map.insert(String::from("fee"), uniswap_v3_pool.fee.into());

                amms_array.push(amm_map.into());
            }
        }
    }

    checkpoint.insert(String::from("amms"), amms_array.into());

    std::fs::write(
        checkpoint_path,
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    )
    .unwrap();
}
