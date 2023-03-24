use std::{
    panic::resume_unwind,
    sync::{Arc, Mutex},
};

use ethers::{
    abi::ParamType,
    providers::Middleware,
    types::{BlockNumber, Log, ValueOrArray, H160, H256, U256},
};
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};

use crate::{
    errors::CFMMError,
    pool::{Pool, UniswapV3Pool},
    throttle::RequestThrottle,
};

use super::DexVariant;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UniswapV3Dex {
    pub factory_address: H160,
    pub creation_block: BlockNumber,
}

pub const POOL_CREATED_EVENT_SIGNATURE: H256 = H256([
    120, 60, 202, 28, 4, 18, 221, 13, 105, 94, 120, 69, 104, 201, 109, 162, 233, 194, 47, 249, 137,
    53, 122, 46, 139, 29, 155, 43, 78, 107, 113, 24,
]);

impl UniswapV3Dex {
    pub fn new(factory_address: H160, creation_block: BlockNumber) -> UniswapV3Dex {
        UniswapV3Dex {
            factory_address,
            creation_block,
        }
    }

    pub const fn pool_created_event_signature(&self) -> H256 {
        POOL_CREATED_EVENT_SIGNATURE
    }

    pub async fn new_pool_from_event<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<Pool, CFMMError<M>> {
        let tokens = ethers::abi::decode(&[ParamType::Uint(32), ParamType::Address], &log.data)?;
        let pair_address = tokens[1].to_owned().into_address().unwrap();
        Pool::new_from_address(pair_address, DexVariant::UniswapV3, middleware).await
    }

    pub fn new_empty_pool_from_event<M: Middleware>(&self, log: Log) -> Result<Pool, CFMMError<M>> {
        let tokens = ethers::abi::decode(&[ParamType::Uint(32), ParamType::Address], &log.data)?;
        let token_a = H160::from(log.topics[0]);
        let token_b = H160::from(log.topics[1]);
        let fee = tokens[0].to_owned().into_uint().unwrap().as_u32();
        let address = tokens[1].to_owned().into_address().unwrap();

        Ok(Pool::UniswapV3(UniswapV3Pool {
            address,
            token_a,
            token_b,
            token_a_decimals: 0,
            token_b_decimals: 0,
            fee,
            liquidity: 0,
            sqrt_price: U256::zero(),
            tick_spacing: 0,
            tick: 0,
            liquidity_net: 0,
        }))
    }

    pub async fn get_all_pools_from_logs<M: 'static + Middleware>(
        self,
        middleware: Arc<M>,
        current_block: BlockNumber,
        request_throttle: Arc<Mutex<RequestThrottle>>,
        progress_bar: ProgressBar,
    ) -> Result<Vec<Pool>, CFMMError<M>> {
        let mut aggregated_pairs: Vec<Pool> = vec![];

        //Define the step for searching a range of blocks for pair created events
        let step = 100000;
        //Unwrap can be used here because the creation block was verified within `Dex::new()`
        let from_block = self
            .creation_block
            .as_number()
            .expect("Error using converting creation block as number")
            .as_u64();
        let current_block = current_block
            .as_number()
            .expect("Error using converting current block as number")
            .as_u64();

        //Initialize the progress bar message
        progress_bar.set_length(current_block - from_block);
        progress_bar.set_message(format!("Getting all pools from: {}", self.factory_address));

        //Init a new vec to keep track of tasks
        let mut handles = vec![];

        //For each block within the range, get all pairs asynchronously
        for from_block in (from_block..=current_block).step_by(step) {
            let request_throttle = request_throttle.clone();
            let provider = middleware.clone();
            let progress_bar = progress_bar.clone();

            //Spawn a new task to get pair created events from the block range
            handles.push(tokio::spawn(async move {
                let mut pools = vec![];

                //Get pair created event logs within the block range
                let to_block = from_block + step as u64;

                //Update the throttle
                request_throttle
                    .lock()
                    .expect("Error when acquiring request throttle mutex lock")
                    .increment_or_sleep(1);

                let logs = provider
                    .get_logs(
                        &ethers::types::Filter::new()
                            .topic0(ValueOrArray::Value(self.pool_created_event_signature()))
                            .address(self.factory_address)
                            .from_block(BlockNumber::Number(ethers::types::U64([from_block])))
                            .to_block(BlockNumber::Number(ethers::types::U64([to_block]))),
                    )
                    .await
                    .map_err(CFMMError::MiddlewareError)?;

                //For each pair created log, create a new Pair type and add it to the pairs vec
                for log in logs {
                    let pool = self.new_empty_pool_from_event(log)?;
                    pools.push(pool);
                }

                //Increment the progress bar by the step
                progress_bar.inc(step as u64);

                Ok::<Vec<Pool>, CFMMError<M>>(pools)
            }));
        }

        //Wait for each thread to finish and aggregate the pairs from each Dex into a single aggregated pairs vec
        for handle in handles {
            match handle.await {
                Ok(sync_result) => aggregated_pairs.extend(sync_result?),

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

        Ok(aggregated_pairs)
    }
}
