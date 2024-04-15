use std::{
    fs::read_to_string,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ethers::providers::Middleware;

use indicatif::MultiProgress;
use serde::{Deserialize, Serialize};

use tokio::{
    sync::Semaphore,
    task::{JoinHandle, JoinSet},
};

use crate::{
    amm::{
        factory::{AutomatedMarketMakerFactory, Factory},
        AMM,
    },
    errors::{AMMError, CheckpointError},
    filters, finish_progress, init_progress, update_progress_by_one,
};

static TASK_PERMITS: Semaphore = Semaphore::const_new(100);

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
) -> Result<(Vec<Factory>, Vec<AMM>, u64), AMMError<M>> {
    tracing::info!("Syncing AMMs from checkpoint");
    let current_block = middleware
        .get_block_number()
        .await
        .map_err(AMMError::MiddlewareError)?
        .as_u64();

    let checkpoint: Checkpoint =
        serde_json::from_str(read_to_string(path_to_checkpoint)?.as_str())?;

    let mut aggregated_amms = sync_amm_data_from_checkpoint(
        checkpoint.amms,
        checkpoint.block_number,
        current_block,
        middleware.clone(),
    )
    .await?;
    let factories = checkpoint.factories.clone();

    let _permit = TASK_PERMITS.acquire().await.unwrap();
    aggregated_amms.extend(
        get_new_amms_from_range(
            factories,
            checkpoint.block_number,
            current_block,
            step,
            middleware.clone(),
        )
        .await?,
    );

    construct_checkpoint(
        checkpoint.factories.clone(),
        &aggregated_amms,
        current_block,
        path_to_checkpoint,
    )?;

    Ok((checkpoint.factories, aggregated_amms, current_block))
}

pub async fn get_new_amms_from_range<M: 'static + Middleware>(
    factories: Vec<Factory>,
    from_block: u64,
    to_block: u64,
    step: u64,
    middleware: Arc<M>,
) -> Result<Vec<AMM>, AMMError<M>> {
    let mut new_amms = vec![];
    let mut join_set = JoinSet::new();
    tracing::info!("Getting new AMMs from range {} to {}", from_block, to_block);
    for factory in factories {
        let middleware = middleware.clone();

        //Spawn a new thread to get all pools and sync data for each dex
        join_set.spawn(async move {
            let mut amms = factory
                .get_all_pools_from_logs(from_block, to_block, step, middleware.clone())
                .await?;

            factory
                .populate_amm_data(&mut amms, Some(to_block), middleware.clone())
                .await?;

            //Clean empty pools
            amms = filters::filter_empty_amms(amms);

            Ok::<_, AMMError<M>>(amms)
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(result) => match result {
                Ok(amms) => {
                    new_amms.extend(amms);
                }
                Err(err) => return Err(err),
            },
            Err(err) => {
                return Err(AMMError::JoinError(err));
            }
        }
    }

    Ok(new_amms)
}

pub async fn sync_amm_data_from_checkpoint<M: 'static + Middleware>(
    amms: Vec<AMM>,
    checkpoint_block: u64,
    to_block: u64,
    middleware: Arc<M>,
) -> Result<Vec<AMM>, AMMError<M>> {
    let multi_progress = MultiProgress::new();
    let progress = multi_progress.add(init_progress!(
        amms.len(),
        "Populating AMM Data from Checkpoint"
    ));
    progress.set_position(0);
    let mut synced_amms = vec![];
    let mut join_set = JoinSet::new();
    for mut amm in amms {
        let middleware = middleware.clone();
        join_set.spawn(async move {
            let _permit = TASK_PERMITS.acquire().await.unwrap();
            match amm {
                AMM::UniswapV2Pool(ref mut pool) => {
                    (pool.reserve_0, pool.reserve_1) =
                        pool.get_reserves(middleware, Some(to_block)).await?;
                }
                AMM::UniswapV3Pool(ref mut pool) => {
                    pool.populate_tick_data(checkpoint_block, Some(to_block), middleware.clone())
                        .await?;
                    pool.sqrt_price = pool
                        .get_sqrt_price(middleware.clone(), Some(to_block))
                        .await?;
                    pool.liquidity = pool
                        .get_liquidity(middleware.clone(), Some(to_block))
                        .await? as i128;
                }
                AMM::ERC4626Vault(ref mut vault) => {
                    (vault.vault_reserve, vault.asset_reserve) =
                        vault.get_reserves(middleware, Some(to_block)).await?;
                }
            }

            Ok::<AMM, AMMError<M>>(amm)
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(result) => match result {
                Ok(amm) => {
                    update_progress_by_one!(progress);
                    synced_amms.push(amm);
                }
                Err(err) => return Err(err),
            },
            Err(err) => {
                tracing::error!(?err);
                return Err(AMMError::JoinError(err));
            }
        }
    }

    finish_progress!(progress);

    Ok(synced_amms)
}

pub fn sort_amms(amms: Vec<AMM>) -> (Vec<AMM>, Vec<AMM>, Vec<AMM>) {
    let mut uniswap_v2_pools = vec![];
    let mut uniswap_v3_pools = vec![];
    let mut erc_4626_vaults = vec![];
    for amm in amms {
        match amm {
            AMM::UniswapV2Pool(_) => uniswap_v2_pools.push(amm),
            AMM::UniswapV3Pool(_) => uniswap_v3_pools.push(amm),
            AMM::ERC4626Vault(_) => erc_4626_vaults.push(amm),
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
) -> Vec<JoinHandle<Result<Vec<AMM>, AMMError<M>>>> {
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
            pools = filters::filter_empty_amms(pools);

            Ok::<_, AMMError<M>>(pools)
        }));
    }

    handles
}

pub fn construct_checkpoint(
    factories: Vec<Factory>,
    amms: &[AMM],
    latest_block: u64,
    checkpoint_path: &str,
) -> Result<(), CheckpointError> {
    let checkpoint = Checkpoint::new(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64() as usize,
        latest_block,
        factories,
        amms.to_vec(),
    );

    std::fs::write(checkpoint_path, serde_json::to_string_pretty(&checkpoint)?)?;

    Ok(())
}

//Deconstructs the checkpoint into a Vec<AMM>
pub fn deconstruct_checkpoint(checkpoint_path: &str) -> Result<(Vec<AMM>, u64), CheckpointError> {
    let checkpoint: Checkpoint = serde_json::from_str(read_to_string(checkpoint_path)?.as_str())?;
    Ok((checkpoint.amms, checkpoint.block_number))
}

#[cfg(test)]
mod test {

    #[tokio::test(flavor = "multi_thread")]
    #[ignore] // requires building `src/sync/checkpoint_test.json` prior to testing.
    pub async fn test_sync_amms_from_checkpoint() -> eyre::Result<()> {
        use crate::amm::AutomatedMarketMaker;
        use crate::sync::{AMM, AMM::UniswapV3Pool};
        use ethers::types::U256;
        use ethers::{
            providers::{Http, Provider},
            types::H160,
        };
        use std::{str::FromStr, sync::Arc};
        abigen!(
            IQuoter,
        r#"[
            function quoteExactInputSingle(address tokenIn, address tokenOut,uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external returns (uint256 amountOut)
        ]"#;);
        use ethers::contract::abigen;
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .compact()
            .init();
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
        let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

        let (_, amms, block) = super::sync_amms_from_checkpoint(
            "src/sync/checkpoint_test.json",
            500,
            provider.clone(),
        )
        .await?;

        // Get usd/eth pool
        let pool = amms
            .iter()
            .find(|amm| {
                if let AMM::UniswapV3Pool(pool) = amm {
                    pool.address
                        == H160::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640").unwrap()
                } else {
                    false
                }
            })
            .unwrap();

        let quoter = IQuoter::new(
            H160::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
            provider.clone(),
        );
        let amount_in = U256::from_dec_str("100000000")?; // 100 USDC
        if let UniswapV3Pool(pool) = pool {
            let amount_out = pool.simulate_swap(pool.token_a, amount_in)?;
            let expected_amount_out = quoter
                .quote_exact_input_single(
                    pool.token_a,
                    pool.token_b,
                    pool.fee,
                    amount_in,
                    U256::zero(),
                )
                .block(block)
                .call()
                .await?;
            assert_eq!(amount_out, expected_amount_out);
        }

        Ok(())
    }
}
