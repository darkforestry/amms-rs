pub mod checkpoint;

use crate::{
    amm::{
        factory::{AutomatedMarketMakerFactory, Factory},
        uniswap_v2, uniswap_v3, AutomatedMarketMaker, AMM,
    },
    errors::AMMError,
    filters,
};

use alloy::{network::Network, providers::Provider, transports::Transport};

use std::{panic::resume_unwind, sync::Arc};

/// Syncs all AMMs from the supplied factories.
///
/// factories - A vector of factories to sync AMMs from.
/// provider - A provider to use for syncing AMMs.
/// checkpoint_path - A path to save a checkpoint of the synced AMMs.
/// step - The step size for batched RPC requests.
/// Returns a tuple of the synced AMMs and the last synced block number.
pub async fn sync_amms<T, N, P>(
    factories: Vec<Factory>,
    provider: Arc<P>,
    checkpoint_path: Option<&str>,
    step: u64,
) -> Result<(Vec<AMM>, u64), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + 'static,
{
    tracing::info!(?step, ?factories, "Syncing AMMs");

    let current_block = provider.get_block_number().await?;

    // Aggregate the populated pools from each thread
    let mut aggregated_amms: Vec<AMM> = vec![];
    let mut handles = vec![];

    // For each dex supplied, get all pair created events and get reserve values
    for factory in factories.clone() {
        let provider = provider.clone();

        // Spawn a new thread to get all pools and sync data for each dex
        handles.push(tokio::spawn(async move {
            tracing::info!(?factory, "Getting all AMMs from factory");
            // Get all of the amms from the factory
            let mut amms = factory
                .get_all_amms(Some(current_block), provider.clone(), step)
                .await?;

            tracing::info!(?factory, "Populating AMMs from factory");
            populate_amms(&mut amms, current_block, provider.clone()).await?;

            // Clean empty pools
            amms = filters::filter_empty_amms(amms);

            // If the factory is UniswapV2, set the fee for each pool according to the factory fee
            if let Factory::UniswapV2Factory(factory) = factory {
                for amm in amms.iter_mut() {
                    if let AMM::UniswapV2Pool(ref mut pool) = amm {
                        pool.fee = factory.fee;
                    }
                }
            }

            Ok::<_, AMMError>(amms)
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

    // Save a checkpoint if a path is provided

    if let Some(checkpoint_path) = checkpoint_path {
        checkpoint::construct_checkpoint(
            factories,
            &aggregated_amms,
            current_block,
            checkpoint_path,
        )?;
    }

    // Return the populated aggregated amms vec
    Ok((aggregated_amms, current_block))
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

// Gets all pool data and sync reserves
pub async fn populate_amms<T, N, P>(
    amms: &mut [AMM],
    block_number: u64,
    provider: Arc<P>,
) -> Result<(), AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    if amms_are_congruent(amms) {
        match amms[0] {
            AMM::UniswapV2Pool(_) => {
                // Max batch size for call
                let step = 127;
                for amm_chunk in amms.chunks_mut(step) {
                    uniswap_v2::batch_request::get_amm_data_batch_request(
                        amm_chunk,
                        provider.clone(),
                    )
                    .await?;
                }
            }

            AMM::UniswapV3Pool(_) => {
                // Max batch size for call
                let step = 76;
                for amm_chunk in amms.chunks_mut(step) {
                    uniswap_v3::batch_request::get_amm_data_batch_request(
                        amm_chunk,
                        block_number,
                        provider.clone(),
                    )
                    .await?;
                }
            }

            // TODO: Implement batch request
            AMM::ERC4626Vault(_) => {
                for amm in amms {
                    amm.populate_data(None, provider.clone()).await?;
                }
            }
        }
    } else {
        return Err(AMMError::IncongruentAMMs);
    }

    // For each pair in the pairs vec, get the pool data
    Ok(())
}
