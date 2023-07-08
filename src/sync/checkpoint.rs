use std::{
    fs::read_to_string,
    panic::resume_unwind,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ethers::{providers::Middleware, types::H160};

use serde::{Deserialize, Serialize};

use tokio::task::JoinHandle;

use crate::{
    amm::{
        factory::{AutomatedMarketMakerFactory, Factory},
        izumi::factory::IziSwapFactory,
        uniswap_v2::factory::UniswapV2Factory,
        uniswap_v3::factory::UniswapV3Factory,
        AMM,
    },
    errors::DAMMError,
    sync,
};

use super::amms_are_congruent;

#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub timestamp: usize,
    pub block_number: u64,
    pub factories: Vec<Factory>,
    pub amms: Vec<AMM>,
}

impl Checkpoint {
    pub fn new(
        timestamp: usize,
        block_number: u64,
        factories: Vec<Factory>,
        amms: Vec<AMM>,
    ) -> Checkpoint {
        Checkpoint {
            timestamp,
            block_number,
            factories,
            amms,
        }
    }
}

//Get all pairs from last synced block and sync reserve values for each Dex in the `dexes` vec.
pub async fn sync_amms_from_checkpoint<M: 'static + Middleware>(
    path_to_checkpoint: &str,
    step: u64,
    middleware: Arc<M>,
) -> Result<(Vec<Factory>, Vec<AMM>), DAMMError<M>> {
    let current_block = middleware
        .get_block_number()
        .await
        .map_err(DAMMError::MiddlewareError)?
        .as_u64();

    let checkpoint: Checkpoint = serde_json::from_str(
        read_to_string(path_to_checkpoint)
            .expect("Error when reading in checkpoint json")
            .as_str(),
    )
    .expect("Error when converting checkpoint file contents to serde_json::Value");

    //Sort all of the pools from the checkpoint into uniswap_v2_pools and uniswap_v3_pools pools so we can sync them concurrently
    let (uniswap_v2_pools, uniswap_v3_pools, _erc_4626_pools) = sort_amms(checkpoint.amms);

    let mut aggregated_amms = vec![];
    let mut handles = vec![];

    //Sync all uniswap v2 pools from checkpoint
    if !uniswap_v2_pools.is_empty() {
        handles.push(
            batch_sync_amms_from_checkpoint(
                uniswap_v2_pools,
                Some(current_block),
                middleware.clone(),
            )
            .await,
        );
    }

    //Sync all uniswap v3 pools from checkpoint
    if !uniswap_v3_pools.is_empty() {
        handles.push(
            batch_sync_amms_from_checkpoint(
                uniswap_v3_pools,
                Some(current_block),
                middleware.clone(),
            )
            .await,
        );
    }

    // TODO: Batch sync erc4626 pools from checkpoint

    //Sync all pools from the since synced block
    handles.extend(
        get_new_amms_from_range(
            checkpoint.factories.clone(),
            checkpoint.block_number,
            current_block,
            step,
            middleware.clone(),
        )
        .await,
    );

    for handle in handles {
        match handle.await {
            Ok(sync_result) => aggregated_amms.extend(sync_result?),
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
        checkpoint.factories.clone(),
        &aggregated_amms,
        current_block,
        path_to_checkpoint,
    );

    Ok((checkpoint.factories, aggregated_amms))
}

pub async fn get_new_amms_from_range<M: 'static + Middleware>(
    factories: Vec<Factory>,
    from_block: u64,
    to_block: u64,
    step: u64,
    middleware: Arc<M>,
) -> Vec<JoinHandle<Result<Vec<AMM>, DAMMError<M>>>> {
    //Create the filter with all the pair created events
    //Aggregate the populated pools from each thread
    let mut handles = vec![];

    for factory in factories.into_iter() {
        let middleware = middleware.clone();

        //Spawn a new thread to get all pools and sync data for each dex
        handles.push(tokio::spawn(async move {
            let mut amms = factory
                .get_all_pools_from_logs(from_block, to_block, step, middleware.clone())
                .await?;

            factory
                .populate_amm_data(&mut amms, Some(to_block), middleware.clone())
                .await?;

            //Clean empty pools
            amms = sync::remove_empty_amms(amms);

            Ok::<_, DAMMError<M>>(amms)
        }));
    }

    handles
}

pub async fn batch_sync_amms_from_checkpoint<M: 'static + Middleware>(
    mut amms: Vec<AMM>,
    block_number: Option<u64>,
    middleware: Arc<M>,
) -> JoinHandle<Result<Vec<AMM>, DAMMError<M>>> {
    let factory = match amms[0] {
        AMM::UniswapV2Pool(_) => Some(Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::zero(),
            0,
            0,
        ))),

        AMM::UniswapV3Pool(_) => Some(Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::zero(),
            0,
        ))),

        AMM::ERC4626Vault(_) => None,
        AMM::IziSwapPool(_) => Some(Factory::IziSwapFactory(IziSwapFactory::new(
            H160::zero(),
            0,
        ))),
    };

    //Spawn a new thread to get all pools and sync data for each dex
    tokio::spawn(async move {
        if let Some(factory) = factory {
            if amms_are_congruent(&amms) {
                //Get all pool data via batched calls
                factory
                    .populate_amm_data(&mut amms, block_number, middleware)
                    .await?;

                //Clean empty pools
                amms = sync::remove_empty_amms(amms);

                Ok::<_, DAMMError<M>>(amms)
            } else {
                Err(DAMMError::IncongruentAMMs)
            }
        } else {
            Ok::<_, DAMMError<M>>(vec![])
        }
    })
}

pub fn sort_amms(amms: Vec<AMM>) -> (Vec<AMM>, Vec<AMM>, Vec<AMM>) {
    let mut uniswap_v2_pools = vec![];
    let mut uniswap_v3_pools = vec![];
    let mut erc_4626_vaults = vec![];
    let mut izi_swap_pools = vec![];
    for amm in amms {
        match amm {
            AMM::UniswapV2Pool(_) => uniswap_v2_pools.push(amm),
            AMM::UniswapV3Pool(_) => uniswap_v3_pools.push(amm),
            AMM::ERC4626Vault(_) => erc_4626_vaults.push(amm),
            AMM::IziSwapPool(_) => izi_swap_pools.push(amm),
        }
    }

    (uniswap_v2_pools, uniswap_v3_pools, erc_4626_vaults)
}

pub async fn get_new_pools_from_range<M: 'static + Middleware>(
    factories: Vec<Factory>,
    from_block: u64,
    to_block: u64,
    step: u64,
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
                .get_all_pools_from_logs(from_block, to_block, step, middleware.clone())
                .await?;

            factory
                .populate_amm_data(&mut pools, Some(to_block), middleware.clone())
                .await?;

            //Clean empty pools
            pools = sync::remove_empty_amms(pools);

            Ok::<_, DAMMError<M>>(pools)
        }));
    }

    handles
}

pub fn construct_checkpoint(
    factories: Vec<Factory>,
    amms: &[AMM],
    latest_block: u64,
    checkpoint_path: &str,
) {
    let checkpoint = Checkpoint::new(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64() as usize,
        latest_block,
        factories,
        amms.to_vec(),
    );

    std::fs::write(
        checkpoint_path,
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    )
    .unwrap();
}

//Deconstructs the checkpoint into a Vec<AMM>
pub fn deconstruct_checkpoint(checkpoint_path: &str) -> Vec<AMM> {
    let checkpoint: Checkpoint = serde_json::from_str(
        read_to_string(checkpoint_path)
            .expect("Error when reading in checkpoint json")
            .as_str(),
    )
    .expect("Error when converting checkpoint file contents to serde_json::Value");

    checkpoint.amms
}
