use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use crate::{
    amm::{
        factory::{AutomatedMarketMakerFactory, TASK_LIMIT},
        AutomatedMarketMaker, AMM,
    },
    errors::{DAMMError, EventLogError},
};
use async_trait::async_trait;
use ethers::{
    abi::RawLog,
    prelude::{abigen, EthEvent},
    providers::Middleware,
    types::{BlockNumber, Filter, Log, H160, H256, U256, U64},
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use super::{batch_request, IziSwapPool};

abigen!(
    IiZiSwapFactory,
    r#"[
        event NewPool(address indexed tokenX,address indexed tokenY,uint24 indexed fee,uint24 pointDelta,address pool)
    ]"#;
);

pub const IZI_POOL_CREATED_EVENT_SIGNATURE: H256 = H256([
    240,
    77,
    166,
    119,
    85,
    173,
    245,
    135,
    57,
    100,
    158,
    47,
    185,
    148,
    154,
    99,
    40,
    81,
    129,
    65,
    183,
    172,
    158,
    68,
    170,
    16,
    50,
    6,
    136,
    176,
    73,
    0,
]);

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IziSwapFactory {
    pub address: H160,
    pub creation_block: u64,
}

#[async_trait]
impl AutomatedMarketMakerFactory for IziSwapFactory {
    fn address(&self) -> H160 {
        self.address
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }

    fn amm_created_event_signature(&self) -> H256 {
        IZI_POOL_CREATED_EVENT_SIGNATURE
    }

    async fn new_amm_from_log<M: 'static + Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, DAMMError<M>> {
        if let Some(block_number) = log.block_number {
            let pool_created_filter = NewPoolFilter::decode_log(&RawLog::from(log))?;
            Ok(AMM::IziSwapPool(
                IziSwapPool::new_from_address(
                    pool_created_filter.pool,
                    block_number.as_u64(),
                    middleware,
                )
                .await?,
            ))
        } else {
            return Err(DAMMError::BlockNumberNotFound);
        }
    }

    async fn get_all_amms<M: 'static + Middleware>(
        &self,
        to_block: Option<u64>,
        middleware: Arc<M>,
        step: u64,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        if let Some(block) = to_block {
            self.get_all_pools_from_logs(block, step, middleware).await
        } else {
            return Err(DAMMError::BlockNumberNotFound);
        }
    }

    async fn populate_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        if let Some(block_number) = block_number {
            let step = 127; //Max batch size for call
            for amm_chunk in amms.chunks_mut(step) {
                batch_request::get_amm_data_batch_request(
                    amm_chunk,
                    block_number,
                    middleware.clone(),
                )
                .await?;
            }
        } else {
            return Err(DAMMError::BlockNumberNotFound);
        }

        Ok(())
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error> {
        let pool_created_event = NewPoolFilter::decode_log(&RawLog::from(log.clone()))?;

        Ok(AMM::IziSwapPool(IziSwapPool {
            address: pool_created_event.pool,
            token_a: pool_created_event.token_x,
            token_b: pool_created_event.token_y,
            token_a_decimals: 0,
            token_b_decimals: 0,
            fee: pool_created_event.fee,
            liquidity: 0,
            sqrt_price: U256::zero(),
            liquidity_x: 0,
            liquidity_y: 0,
            current_point: 0,
            point_delta: 0,
        }))
    }
}

impl IziSwapFactory {
    pub fn new(address: H160, creation_block: u64) -> IziSwapFactory {
        IziSwapFactory {
            address,
            creation_block,
        }
    }

    //Function to get all pair created events for a given Dex factory address and sync pool data
    pub async fn get_all_pools_from_logs<M: 'static + Middleware>(
        self,
        to_block: u64,
        step: u64,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        //Unwrap can be used here because the creation block was verified within `Dex::new()`
        let mut from_block = self.creation_block;
        let mut aggregated_amms: HashMap<H160, AMM> = HashMap::new();

        //TODO: NOTE: we do not currently need to use a btreemap but if we are to do local simulation we will need burn and mint logs. For this reason, we are leaving the sync arch to be the same as uniswapV3
        let mut ordered_logs: BTreeMap<U64, Vec<Log>> = BTreeMap::new();

        let mut handles = vec![];

        let mut tasks = 0;
        while from_block < to_block {
            let middleware = middleware.clone();

            let mut target_block = from_block + step - 1;
            if target_block > to_block {
                target_block = to_block;
            }

            handles.push(tokio::spawn(async move {
                let logs = middleware
                    .get_logs(
                        &Filter::new()
                            .topic0(vec![IZI_POOL_CREATED_EVENT_SIGNATURE])
                            .from_block(BlockNumber::Number(U64([from_block])))
                            .to_block(BlockNumber::Number(U64([target_block]))),
                    )
                    .await
                    .map_err(DAMMError::MiddlewareError)?;

                Ok::<Vec<Log>, DAMMError<M>>(logs)
            }));

            from_block += step;

            tasks += 1;
            //Here we are limiting the number of green threads that can be spun up to not have the node time out
            if tasks == TASK_LIMIT {
                self.process_logs_from_handles(handles, &mut ordered_logs)
                    .await?;
                handles = vec![];
                tasks = 0;
            }
        }

        self.process_logs_from_handles(handles, &mut ordered_logs)
            .await?;

        for (_, log_group) in ordered_logs {
            for log in log_group {
                let event_signature = log.topics[0];

                //If the event sig is the pool created event sig, then the log is coming from the factory
                if event_signature == IZI_POOL_CREATED_EVENT_SIGNATURE && log.address == self.address {
                    let new_pool = self.new_empty_amm_from_log(log)?;

                    aggregated_amms.insert(new_pool.address(), new_pool);
                }
            }
        }

        Ok(aggregated_amms.into_values().collect::<Vec<AMM>>())
    }

    async fn process_logs_from_handles<M: Middleware>(
        &self,
        handles: Vec<JoinHandle<Result<Vec<Log>, DAMMError<M>>>>,
        ordered_logs: &mut BTreeMap<U64, Vec<Log>>,
    ) -> Result<(), DAMMError<M>> {
        // group the logs from each thread by block number and then sync the logs in chronological order
        for handle in handles {
            let logs = handle.await??;

            for log in logs {
                if let Some(log_block_number) = log.block_number {
                    if let Some(log_group) = ordered_logs.get_mut(&log_block_number) {
                        log_group.push(log);
                    } else {
                        ordered_logs.insert(log_block_number, vec![log]);
                    }
                } else {
                    return Err(EventLogError::LogBlockNumberNotFound)?;
                }
            }
        }
        Ok(())
    }
}
