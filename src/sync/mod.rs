use crate::{
    amm::{
        factory::{AutomatedMarketMakerFactory, Factory},
        uniswap_v3, AMM,
    },
    errors::DAMMError,
};

use ethers::providers::Middleware;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    panic::resume_unwind,
    sync::{Arc, Mutex},
};
pub mod checkpoint;

pub async fn sync_amms<M: 'static + Middleware>(
    factories: Vec<Factory>,
    step: usize,
    middleware: Arc<M>,
    checkpoint_path: Option<&str>,
) -> Result<Vec<AMM>, DAMMError<M>> {
    let current_block = middleware
        .get_block_number()
        .await
        .map_err(DAMMError::MiddlewareError)?;

    //Aggregate the populated pools from each thread
    let mut aggregated_amms: Vec<AMM> = vec![];
    let mut handles = vec![];

    //For each dex supplied, get all pair created events and get reserve values
    for factory in factories.clone() {
        let middleware = middleware.clone();

        //Spawn a new thread to get all pools and sync data for each dex
        handles.push(tokio::spawn(async move {
            //Get all of the amms from the factory
            let mut amms: Vec<AMM> = factory.get_all_amms(middleware.clone()).await?;
            populate_amms(&mut amms, middleware.clone()).await?;
            //Clean empty pools
            amms = remove_empty_amms(amms);
            Ok::<_, DAMMError<M>>(amms)
        }));
    }

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

    //Save a checkpoint if a path is provided
    if checkpoint_path.is_some() {
        let checkpoint_path = checkpoint_path.unwrap();

        checkpoint::construct_checkpoint(
            factories,
            &aggregated_amms,
            current_block.as_u64(),
            checkpoint_path,
        )
    }

    //Return the populated aggregated amms vec
    Ok(aggregated_amms)
}

pub fn amms_are_congruent(amms: &[AMM]) -> bool {
    let expected_amm = amms[0];

    for amm in amms {
        if std::mem::discriminant(&expected_amm) != std::mem::discriminant(amm) {
            return false;
        }
    }
    return true;
}

//Gets all pool data and sync reserves
pub async fn populate_amms<M: Middleware>(
    amms: &mut [AMM],
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    if amms_are_congruent(&amms) {
        match amms[0] {
            AMM::UniswapV2Pool(_) => {
                let step = 127; //Max batch size for call
                for amm_chunk in amms.chunks_mut(step) {
                    uniswap_v3::batch_request::get_amm_data_batch_request(
                        amm_chunk,
                        middleware.clone(),
                    )
                    .await?;

                    // progress_bar.inc(step as u64);
                }
            }

            AMM::UniswapV3Pool(_) => {
                let step = 76; //Max batch size for call
                for amm_chunk in amms.chunks_mut(step) {
                    uniswap_v3::batch_request::get_amm_data_batch_request(
                        amm_chunk,
                        middleware.clone(),
                    )
                    .await?;

                    // progress_bar.inc(step as u64);
                }
            }
        }
    } else {
        return Err(DAMMError::IncongruentAMMs);
    }

    //For each pair in the pairs vec, get the pool data
    Ok(())
}

pub fn remove_empty_amms(pools: Vec<AMM>) -> Vec<AMM> {
    let mut cleaned_pools = vec![];

    for pool in pools {
        match pool {
            AMM::UniswapV2Pool(uniswap_v2_pool) => {
                if !uniswap_v2_pool.token_a.is_zero() {
                    cleaned_pools.push(pool)
                }
            }
            AMM::UniswapV3Pool(uniswap_v3_pool) => {
                if !uniswap_v3_pool.token_a.is_zero() {
                    cleaned_pools.push(pool)
                }
            }
        }
    }

    cleaned_pools
}
