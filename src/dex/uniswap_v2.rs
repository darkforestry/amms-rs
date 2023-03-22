use std::sync::{Arc, Mutex};

use ethers::{
    abi::ParamType,
    providers::Middleware,
    types::{BlockNumber, Log, H160, H256, U256},
};
use indicatif::ProgressBar;

use crate::{
    abi, batch_requests,
    errors::CFMMError,
    pool::{Pool, UniswapV2Pool},
    throttle::RequestThrottle,
};

use super::DexVariant;

#[derive(Debug, Clone, Copy)]
pub struct UniswapV2Dex {
    pub factory_address: H160,
    pub creation_block: BlockNumber,
    pub fee: u64,
}

pub const PAIR_CREATED_EVENT_SIGNATURE: H256 = H256([
    13, 54, 72, 189, 15, 107, 168, 1, 52, 163, 59, 169, 39, 90, 197, 133, 217, 211, 21, 240, 173,
    131, 85, 205, 222, 253, 227, 26, 250, 40, 208, 233,
]);

impl UniswapV2Dex {
    pub fn new(factory_address: H160, creation_block: BlockNumber, fee: u64) -> UniswapV2Dex {
        UniswapV2Dex {
            factory_address,
            creation_block,
            fee,
        }
    }

    pub const fn pool_created_event_signature(&self) -> H256 {
        PAIR_CREATED_EVENT_SIGNATURE
    }

    pub async fn new_pool_from_event<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<Pool, CFMMError<M>> {
        let tokens = ethers::abi::decode(&[ParamType::Address, ParamType::Uint(256)], &log.data)?;
        let pair_address = tokens[0].to_owned().into_address().unwrap();
        Pool::new_from_address(pair_address, DexVariant::UniswapV2, middleware).await
    }

    pub fn new_empty_pool_from_event<M: Middleware>(&self, log: Log) -> Result<Pool, CFMMError<M>> {
        let tokens = ethers::abi::decode(&[ParamType::Address, ParamType::Uint(256)], &log.data)?;
        let token_a = H160::from(log.topics[0]);
        let token_b = H160::from(log.topics[1]);
        let address = tokens[0].to_owned().into_address().unwrap();

        Ok(Pool::UniswapV2(UniswapV2Pool {
            address,
            token_a,
            token_b,
            token_a_decimals: 0,
            token_b_decimals: 0,
            reserve_0: 0,
            reserve_1: 0,
            fee: 300,
        }))
    }

    pub async fn get_all_pairs_via_batched_calls<M: 'static + Middleware>(
        self,
        middleware: Arc<M>,
        request_throttle: Arc<Mutex<RequestThrottle>>,
        progress_bar: ProgressBar,
    ) -> Result<Vec<Pool>, CFMMError<M>> {
        let factory = abi::IUniswapV2Factory::new(self.factory_address, middleware.clone());

        let pairs_length: U256 = factory.all_pairs_length().call().await?;
        //Initialize the progress bar message
        progress_bar.set_length(pairs_length.as_u64());

        let mut pairs = vec![];
        let step = 766; //max batch size for this call until codesize is too large
        let mut idx_from = U256::zero();
        let mut idx_to = if step > pairs_length.as_usize() {
            pairs_length
        } else {
            U256::from(step)
        };

        for _ in (0..pairs_length.as_u128()).step_by(step) {
            request_throttle
                .lock()
                .expect("Could not acquire mutex")
                .increment_or_sleep(1);

            pairs.append(
                &mut batch_requests::uniswap_v2::get_pairs_batch_request(
                    self.factory_address,
                    idx_from,
                    idx_to,
                    middleware.clone(),
                )
                .await?,
            );

            idx_from = idx_to;

            if idx_to + step > pairs_length {
                idx_to = pairs_length - 1
            } else {
                idx_to = idx_to + step;
            }

            progress_bar.inc(step as u64);
        }

        let mut pools = vec![];

        //Create new empty pools for each pair
        for addr in pairs {
            let pool = UniswapV2Pool {
                address: addr,
                ..Default::default()
            };

            pools.push(Pool::UniswapV2(pool));
        }

        Ok(pools)
    }
}
