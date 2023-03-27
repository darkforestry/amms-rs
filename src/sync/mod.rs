use crate::{
    amm::AMM,
    batch_requests,
    errors::DAMMError,
    factory::{AutomatedMarketMakerFactory, Factory},
};

use ethers::providers::Middleware;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    panic::resume_unwind,
    sync::{Arc, Mutex},
};

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
            let mut amms = factory.get_all_amms(step, middleware.clone()).await?;
            populate_amm_data(&mut amms, middleware.clone()).await?;
            //Clean empty pools
            amms = remove_empty_pools(amms);
            Ok::<_, DAMMError<M>>(amms)
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

    //Save a checkpoint if a path is provided
    if checkpoint_path.is_some() {
        let checkpoint_path = checkpoint_path.unwrap();

        checkpoint::construct_checkpoint(
            dexes,
            &aggregated_pools,
            current_block.as_u64(),
            checkpoint_path,
        )
    }

    //Return the populated aggregated pools vec
    Ok(aggregated_pools)
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
pub async fn populate_amm_data<M: Middleware>(
    amms: &mut [AMM],
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    if amms_are_congruent(&amms) {
        match amms[0] {
            AMM::UniswapV2Pool(_) => {
                let step = 127; //Max batch size for call
                for amm_chunk in amms.chunks_mut(step) {
                    batch_requests::uniswap_v2::get_amm_data_batch_request(
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
                    batch_requests::uniswap_v3::get_amm_data_batch_request(
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

//Get all pairs and sync reserve values for each Dex in the `dexes` vec.
pub async fn sync_pairs_with_throttle<M: 'static + Middleware>(
    dexes: Vec<Dex>,
    step: usize, //TODO: Add docs on step. Step is the block range used to get all pools from a dex if syncing from event logs
    middleware: Arc<M>,
    requests_per_second_limit: usize,
    checkpoint_path: Option<&str>,
) -> Result<Vec<Pool>, CFMMError<M>> {
    let current_block = middleware
        .get_block_number()
        .await
        .map_err(CFMMError::MiddlewareError)?;

    //Initialize a new request throttle
    let request_throttle = Arc::new(Mutex::new(RequestThrottle::new(requests_per_second_limit)));

    //Aggregate the populated pools from each thread
    let mut aggregated_pools: Vec<Pool> = vec![];
    let mut handles = vec![];

    //Initialize multi progress bar
    let multi_progress_bar = MultiProgress::new();

    //For each dex supplied, get all pair created events and get reserve values
    for dex in dexes.clone() {
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
            progress_bar.set_message(format!("Getting all pools from: {}", dex.factory_address()));

            let mut pools = dex
                .get_all_pools(
                    request_throttle.clone(),
                    step,
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
                "Getting all pool data for: {}",
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
            pools = remove_empty_pools(pools);

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

    //Save a checkpoint if a path is provided
    if checkpoint_path.is_some() {
        let checkpoint_path = checkpoint_path.unwrap();

        checkpoint::construct_checkpoint(
            dexes,
            &aggregated_pools,
            current_block.as_u64(),
            checkpoint_path,
        )
    }

    //Return the populated aggregated pools vec
    Ok(aggregated_pools)
}

pub fn remove_empty_pools(pools: Vec<Pool>) -> Vec<Pool> {
    let mut cleaned_pools = vec![];

    for pool in pools {
        match pool {
            Pool::UniswapV2(uniswap_v2_pool) => {
                if !uniswap_v2_pool.token_a.is_zero() {
                    cleaned_pools.push(pool)
                }
            }
            Pool::UniswapV3(uniswap_v3_pool) => {
                if !uniswap_v3_pool.token_a.is_zero() {
                    cleaned_pools.push(pool)
                }
            }
        }
    }

    cleaned_pools
}
