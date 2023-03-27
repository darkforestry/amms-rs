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
use serde_json::{Map, Value};
use tokio::task::JoinHandle;

use crate::{
    amm::AMM,
    dex::{Dex, DexVariant},
    errors::CFMMError,
    factory::{AutomatedMarketMakerFactory, Factory},
    pool::{Pool, UniswapV2Pool, UniswapV3Pool},
    sync,
    throttle::RequestThrottle,
};

//Get all pairs and sync reserve values for each Dex in the `dexes` vec.
pub async fn sync_pools_from_checkpoint<M: 'static + Middleware>(
    path_to_checkpoint: &str,
    step: usize,
    middleware: Arc<M>,
) -> Result<(Vec<Dex>, Vec<Pool>), CFMMError<M>> {
    sync_pools_from_checkpoint_with_throttle(path_to_checkpoint, step, 0, middleware).await
}

//Get all pairs from last synced block and sync reserve values for each Dex in the `dexes` vec.
pub async fn sync_pools_from_checkpoint_with_throttle<M: 'static + Middleware>(
    path_to_checkpoint: &str,
    step: usize,
    requests_per_second_limit: usize,
    middleware: Arc<M>,
) -> Result<(Vec<Dex>, Vec<Pool>), CFMMError<M>> {
    let current_block = middleware
        .get_block_number()
        .await
        .map_err(CFMMError::MiddlewareError)?;

    let request_throttle = Arc::new(Mutex::new(RequestThrottle::new(requests_per_second_limit)));
    //Initialize multi progress bar
    let multi_progress_bar = MultiProgress::new();

    //Read in checkpoint
    let (dexes, pools, checkpoint_block_number) = deconstruct_checkpoint(path_to_checkpoint);

    //Sort all of the pools from the checkpoint into uniswapv2 and uniswapv3 pools so we can sync them concurrently
    let (uinswap_v2_pools, uniswap_v3_pools) = sort_pool_variants(pools);

    let mut aggregated_pools = vec![];
    let mut handles = vec![];

    //Sync all uniswap v2 pools from checkpoint
    if !uinswap_v2_pools.is_empty() {
        handles.push(
            batch_sync_pools_from_checkpoint(
                uinswap_v2_pools,
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

pub async fn batch_sync_pools_from_checkpoint<M: 'static + Middleware>(
    mut pools: Vec<Pool>,
    dex_variant: DexVariant,
    progress_bar: ProgressBar,
    request_throttle: Arc<Mutex<RequestThrottle>>,
    middleware: Arc<M>,
) -> JoinHandle<Result<Vec<Pool>, CFMMError<M>>> {
    let dex = Dex::new(H160::zero(), dex_variant, 0, None);

    //Spawn a new thread to get all pools and sync data for each dex
    tokio::spawn(async move {
        progress_bar.set_style(
            ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7}")
                .expect("Error when setting progress bar style")
                .progress_chars("##-"),
        );
        progress_bar.set_length(pools.len() as u64);

        match dex {
            Dex::UniswapV2(_) => {
                progress_bar.set_message("Syncing all Uniswap V2 pool variants from checkpoint");
            }
            Dex::UniswapV3(_) => {
                progress_bar.set_message("Syncing all Uniswap V3 pool variants from checkpoint");
            }
        }

        //Get all pool data via batched calls
        dex.get_all_pool_data(&mut pools, request_throttle, progress_bar, middleware)
            .await?;

        //Clean empty pools
        pools = sync::remove_empty_pools(pools);

        Ok::<_, CFMMError<M>>(pools)
    })
}

pub fn sort_pool_variants(pools: Vec<Pool>) -> (Vec<Pool>, Vec<Pool>) {
    let mut uniswap_v2_pools = vec![];
    let mut uniswap_v3_pools = vec![];

    for pool in pools {
        match pool {
            Pool::UniswapV2(_) => uniswap_v2_pools.push(pool),
            Pool::UniswapV3(_) => uniswap_v3_pools.push(pool),
        }
    }

    (uniswap_v2_pools, uniswap_v3_pools)
}

pub async fn get_new_pools_from_range<M: 'static + Middleware>(
    dexes: Vec<Dex>,
    from_block: BlockNumber,
    to_block: BlockNumber,
    step: usize,
    request_throttle: Arc<Mutex<RequestThrottle>>,
    multi_progress_bar: MultiProgress,
    middleware: Arc<M>,
) -> Vec<JoinHandle<Result<Vec<Pool>, CFMMError<M>>>> {
    //Create the filter with all the pair created events
    //Aggregate the populated pools from each thread
    let mut handles = vec![];

    for dex in dexes {
        let middleware = middleware.clone();
        let request_throttle = request_throttle.clone();
        let progress_bar = multi_progress_bar.add(ProgressBar::new(0));

        //Spawn a new thread to get all pools and sync data for each dex
        handles.push(tokio::spawn(async move {
            progress_bar.set_style(
                ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7}")
                    .expect("Error when setting progress bar style")
                    .progress_chars("##-"),
            );

            //Get all of the pools from the dex
            progress_bar.set_message(format!(
                "Getting new all pools from: {}",
                dex.factory_address()
            ));

            let mut pools = dex
                .get_all_pools_from_logs_within_range(
                    from_block,
                    to_block,
                    step,
                    request_throttle.clone(),
                    progress_bar.clone(),
                    middleware.clone(),
                )
                .await?;

            progress_bar.reset();
            progress_bar.set_style(
                ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7}")
                    .expect("Error when setting progress bar style")
                    .progress_chars("##-"),
            );

            //Get all of the pool data and sync the pool
            progress_bar.set_message(format!(
                "Getting all pool data for new pools from: {}",
                dex.factory_address()
            ));

            progress_bar.set_length(pools.len() as u64);

            dex.get_all_pool_data(
                &mut pools,
                request_throttle.clone(),
                progress_bar.clone(),
                middleware.clone(),
            )
            .await?;

            //Clean empty pools
            pools = sync::remove_empty_pools(pools);

            Ok::<_, CFMMError<M>>(pools)
        }));
    }

    handles
}

//Get all pairs and sync reserve values for each Dex in the `dexes` vec.
pub async fn generate_checkpoint<M: 'static + Middleware>(
    dexes: Vec<Dex>,
    middleware: Arc<M>,
    checkpoint_file_name: &str,
) -> Result<(), CFMMError<M>> {
    //Sync pairs with throttle but set the requests per second limit to 0, disabling the throttle.
    generate_checkpoint_with_throttle(dexes, middleware, 100000, 0, checkpoint_file_name).await
}

//Get all pairs and sync reserve values for each Dex in the `dexes` vec.
pub async fn generate_checkpoint_with_throttle<M: 'static + Middleware>(
    dexes: Vec<Dex>,
    middleware: Arc<M>,
    step: usize,
    requests_per_second_limit: usize,
    checkpoint_file_name: &str,
) -> Result<(), CFMMError<M>> {
    //Initialize a new request throttle
    let request_throttle = Arc::new(Mutex::new(RequestThrottle::new(requests_per_second_limit)));

    //Aggregate the populated pools from each thread
    let mut aggregated_pools: Vec<Pool> = vec![];
    let mut handles = vec![];

    //Initialize multi progress bar
    let multi_progress_bar = MultiProgress::new();

    //For each dex supplied, get all pair created events and get reserve values
    for dex in dexes.clone() {
        let async_provider = middleware.clone();
        let request_throttle = request_throttle.clone();
        let progress_bar = multi_progress_bar.add(ProgressBar::new(0));

        handles.push(tokio::spawn(async move {
            progress_bar.set_style(
                ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7} Blocks")
                    .unwrap()
                    .progress_chars("##-"),
            );

            let mut pools = dex
                .get_all_pools(
                    request_throttle.clone(),
                    step,
                    progress_bar.clone(),
                    async_provider.clone(),
                )
                .await?;

            progress_bar.reset();
            progress_bar.set_style(
                ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7} Pairs")
                    .unwrap()
                    .progress_chars("##-"),
            );

            dex.get_all_pool_data(
                &mut pools,
                request_throttle.clone(),
                progress_bar.clone(),
                async_provider.clone(),
            )
            .await?;

            progress_bar.finish_and_clear();
            progress_bar.set_message(format!(
                "Finished syncing pools for {} ✅",
                dex.factory_address()
            ));

            progress_bar.finish();

            Ok::<_, CFMMError<M>>(pools)
        }));
    }

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

    //Clean empty pools
    aggregated_pools = sync::remove_empty_pools(aggregated_pools);

    let latest_block = middleware
        .get_block_number()
        .await
        .map_err(CFMMError::MiddlewareError)?;

    println!("total pools :{}", aggregated_pools.len());

    construct_checkpoint(
        dexes,
        &aggregated_pools,
        latest_block.as_u64(),
        checkpoint_file_name,
    );

    Ok(())
}

pub fn deconstruct_checkpoint(checkpoint_path: &str) -> (Vec<Factory>, Vec<AMM>, BlockNumber) {
    let mut factories = vec![];

    let checkpoint_json: serde_json::Value = serde_json::from_str(
        read_to_string(checkpoint_path)
            .expect("Error when reading in checkpoint json")
            .as_str(),
    )
    .expect("Error when converting checkpoint file contents to serde_json::Value");

    let block_number = checkpoint_json
        .get("block_number")
        .expect("Could not get block_number from checkpoint")
        .as_u64()
        .expect("Could not convert block_number to u64");

    for factory in checkpoint_json
        .get("factories")
        .expect("Could not get checkpoint_data")
        .as_array()
        .expect("Could not unwrap checkpoint json into array")
        .iter()
    {
        let factory = deconstruct_dex_from_checkpoint(
            factory
                .as_object()
                .expect("Dex checkpoint is not formatted correctly"),
        );

        factories.push(factory);
    }

    //get all pools
    let pools_array = checkpoint_json
        .get("pools")
        .expect("Could not get pools from checkpoint")
        .as_array()
        .expect("Could not convert pools to value array");

    let pools = deconstruct_pools_from_checkpoint(pools_array);

    (dexes, pools, BlockNumber::Number(block_number.into()))
}

pub fn deconstruct_factory_from_checkpoint(dex_map: &Map<String, Value>) -> Dex {
    

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

pub fn deconstruct_pools_from_checkpoint(pools_array: &Vec<Value>) -> Vec<Pool> {
    let mut pools = vec![];

    for pool_value in pools_array {
        let pool_map = pool_value
            .as_object()
            .expect("Could not convert pool value to map");

        let pool_dex_variant = match pool_map
            .get("dex_variant")
            .expect("Could not get pool dex_variant")
            .as_str()
            .expect("Could not convert dex_variant to str")
            .to_lowercase()
            .as_str()
        {
            "uniswapv2" => DexVariant::UniswapV2,
            "uniswapv3" => DexVariant::UniswapV3,
            _ => {
                panic!("Unrecognized pool dex variant")
            }
        };

        match pool_dex_variant {
            DexVariant::UniswapV2 | DexVariant::UniswapV3 => {
                let addr = H160::from_str(
                    pool_map
                        .get("address")
                        .unwrap_or_else(|| panic!("Could not get pool address {:?}", pool_map))
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!("Could not convert pool address to str {:?}", pool_map)
                        }),
                )
                .expect("Could not convert token_a to H160");

                let token_a = H160::from_str(
                    pool_map
                        .get("token_a")
                        .unwrap_or_else(|| panic!("Could not get token_a {:?}", pool_map))
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!("Could not convert token_a to str {:?}", pool_map)
                        }),
                )
                .expect("Could not convert token_a to H160");

                let token_a_decimals = pool_map
                    .get("token_a_decimals")
                    .unwrap_or_else(|| panic!("Could not get token_a_decimals {:?}", pool_map))
                    .as_u64()
                    .expect("Could not convert token_a_decimals to u64")
                    as u8;

                let token_b = H160::from_str(
                    pool_map
                        .get("token_b")
                        .unwrap_or_else(|| panic!("Could not get token_b {:?}", pool_map))
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!("Could not convert token_b to str {:?}", pool_map)
                        }),
                )
                .expect("Could not convert token_b to H160");

                let token_b_decimals = pool_map
                    .get("token_b_decimals")
                    .unwrap_or_else(|| panic!("Could not get token_b_decimals {:?}", pool_map))
                    .as_u64()
                    .expect("Could not convert token_b_decimals to u64")
                    as u8;

                let fee = pool_map
                    .get("fee")
                    .unwrap_or_else(|| panic!("Could not get fee {:?}", pool_map))
                    .as_u64()
                    .expect("Could not convert fee to u64") as u32;

                match pool_dex_variant {
                    DexVariant::UniswapV2 => {
                        pools.push(Pool::UniswapV2(UniswapV2Pool::new(
                            addr,
                            token_a,
                            token_a_decimals,
                            token_b,
                            token_b_decimals,
                            0,
                            0,
                            fee,
                        )));
                    }

                    DexVariant::UniswapV3 => {
                        pools.push(Pool::UniswapV3(UniswapV3Pool::new(
                            addr,
                            token_a,
                            token_a_decimals,
                            token_b,
                            token_b_decimals,
                            fee,
                            0,
                            U256::zero(),
                            0,
                            0,
                            0,
                        )));
                    }
                }
            }
        }
    }

    pools
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
