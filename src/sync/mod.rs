use crate::{
    amm::{
        factory::{AutomatedMarketMakerFactory, Factory},
        uniswap_v2, uniswap_v3, AutomatedMarketMaker, AMM,
    },
    errors::DAMMError,
};

use ethers::providers::Middleware;

use spinoff::{spinners, Color, Spinner};
use std::{panic::resume_unwind, sync::Arc};
pub mod checkpoint;

//TODO: This needs to return the block that everything started syncing at

pub async fn sync_amms<M: 'static + Middleware>(
    factories: Vec<Factory>,
    middleware: Arc<M>,
    checkpoint_path: Option<&str>,
) -> Result<Vec<AMM>, DAMMError<M>> {
    let spinner = Spinner::new(spinners::Dots, "Syncing AMMs...", Color::Blue);

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
            let mut amms: Vec<AMM> = factory.get_all_amms(
                
               Hello gentlemen, before you undertake this, there are a few things to consider. Because we are now conducting v3 swap simulation completely locally,
                We need to ensure that we do not miss any logs that will result in state changes to a v3 pool, and equally as important, we need to ensure that we do not 
                duplicate the state changes from any log. With the current setup, we would be certainly missing logs from the initial sync to the time that we are listening for changes
                from new blocks. This is due to state space filtering and any other latencies that we might incur. So solve for this, the sync amms function should return a block number that was originally cached
                at the start of the function. This block number should also be passed into `factory.get_all_amms` so that if a factory needs to sync from logs, it will sync from 
                genesis to the passed in block. Then we can return that block and ensure that when we are syncing logs, we start from the block that syncing originally left off at. 
                There are many footguns in this process now that we are doing everything locally and we need to keep perfect state (unwinding state changes, missed blocks, etc) else we will be running
                a glorified space heater instead of an arb machine. 

                In summary, we need to cache a block and pass it into get all amms, as well as return that block. We need to remember to use that block as the first from block when syncing logs after initial amm sync. 
                In the future it would be nice to abstract all of this and keep it all in damms without having to worry about it being replicated across codebases. We could also create another lib that is dedicated towards
                keeping the state and handling all state changes, unwinds, etc from a state space. It could be called something like state_space_something. 

                - Kit
                
                middleware.clone()).await?;
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
    spinner.success("AMMs synced");

    //Return the populated aggregated amms vec
    Ok(aggregated_amms)
}

pub fn amms_are_congruent(amms: &[AMM]) -> bool {
    let expected_amm = &amms[0];

    for amm in amms {
        if std::mem::discriminant(expected_amm) != std::mem::discriminant(amm) {
            return false;
        }
    }
    true
}

//Gets all pool data and sync reserves
pub async fn populate_amms<M: Middleware>(
    amms: &mut [AMM],
    middleware: Arc<M>,
) -> Result<(), DAMMError<M>> {
    if amms_are_congruent(amms) {
        match amms[0] {
            AMM::UniswapV2Pool(_) => {
                let step = 127; //Max batch size for call
                for amm_chunk in amms.chunks_mut(step) {
                    uniswap_v2::batch_request::get_amm_data_batch_request(
                        amm_chunk,
                        middleware.clone(),
                    )
                    .await?;
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
                }
            }

            // TODO: Implement batch request
            AMM::ERC4626Vault(_) => {
                for amm in amms {
                    amm.populate_data(middleware.clone()).await?;
                }
            }
        }
    } else {
        return Err(DAMMError::IncongruentAMMs);
    }

    //For each pair in the pairs vec, get the pool data
    Ok(())
}

pub fn remove_empty_amms(amms: Vec<AMM>) -> Vec<AMM> {
    let mut cleaned_amms = vec![];

    for amm in amms.into_iter() {
        match amm {
            AMM::UniswapV2Pool(ref uniswap_v2_pool) => {
                if !uniswap_v2_pool.token_a.is_zero() && !uniswap_v2_pool.token_b.is_zero() {
                    cleaned_amms.push(amm)
                }
            }
            AMM::UniswapV3Pool(ref uniswap_v3_pool) => {
                if !uniswap_v3_pool.token_a.is_zero() && !uniswap_v3_pool.token_b.is_zero() {
                    cleaned_amms.push(amm)
                }
            }
            AMM::ERC4626Vault(ref erc4626_vault) => {
                if !erc4626_vault.vault_token.is_zero() && !erc4626_vault.asset_token.is_zero() {
                    cleaned_amms.push(amm)
                }
            }
        }
    }

    cleaned_amms
}
