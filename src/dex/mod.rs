use std::sync::{Arc, Mutex};

use ethers::{
    providers::Middleware,
    types::{BlockNumber, Filter, Log, ValueOrArray, H160, H256, U64},
};
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};

use crate::{
    abi, batch_requests,
    errors::CFMMError,
    pool::{Pool, UniswapV2Pool, UniswapV3Pool},
    throttle::RequestThrottle,
};

use self::{uniswap_v2::UniswapV2Dex, uniswap_v3::UniswapV3Dex};

pub mod uniswap_v2;
pub mod uniswap_v3;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Dex {
    UniswapV2(UniswapV2Dex),
    UniswapV3(UniswapV3Dex),
}

impl Dex {
    pub fn new(
        factory_address: H160,
        dex_variant: DexVariant,
        creation_block: u64,
        fee: Option<u64>,
    ) -> Dex {
        let fee = if let Some(fee) = fee { fee } else { 300 };

        match dex_variant {
            DexVariant::UniswapV2 => Dex::UniswapV2(UniswapV2Dex::new(
                factory_address,
                BlockNumber::Number(creation_block.into()),
                fee,
            )),

            DexVariant::UniswapV3 => Dex::UniswapV3(UniswapV3Dex::new(
                factory_address,
                BlockNumber::Number(creation_block.into()),
            )),
        }
    }

    pub fn factory_address(&self) -> H160 {
        match self {
            Dex::UniswapV2(uniswap_v2_dex) => uniswap_v2_dex.factory_address,
            Dex::UniswapV3(uniswap_v3_dex) => uniswap_v3_dex.factory_address,
        }
    }

    pub fn creation_block(&self) -> BlockNumber {
        match self {
            Dex::UniswapV2(uniswap_v2_dex) => uniswap_v2_dex.creation_block,
            Dex::UniswapV3(uniswap_v3_dex) => uniswap_v3_dex.creation_block,
        }
    }

    pub fn pool_created_event_signature(&self) -> H256 {
        match self {
            Dex::UniswapV2(uniswap_v2_dex) => uniswap_v2_dex.pool_created_event_signature(),
            Dex::UniswapV3(uniswap_v3_dex) => uniswap_v3_dex.pool_created_event_signature(),
        }
    }

    pub async fn new_pool_from_event_log<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<Pool, CFMMError<M>> {
        Pool::new_from_event_log(log, middleware).await
    }

    pub async fn get_all_pools<M: 'static + Middleware>(
        &self,
        request_throttle: Arc<Mutex<RequestThrottle>>,
        step: usize,
        progress_bar: ProgressBar,
        middleware: Arc<M>,
    ) -> Result<Vec<Pool>, CFMMError<M>> {
        match self {
            Dex::UniswapV2(uniswap_v2_dex) => {
                uniswap_v2_dex
                    .get_all_pairs_via_batched_calls(middleware, request_throttle, progress_bar)
                    .await
            }
            Dex::UniswapV3(_) => {
                let current_block = middleware
                    .get_block_number()
                    .await
                    .map_err(CFMMError::MiddlewareError)?;

                self.get_all_pools_from_logs(
                    current_block.into(),
                    step,
                    request_throttle,
                    progress_bar,
                    middleware,
                )
                .await
            }
        }
    }

    //Gets all pool data and sync reserves
    pub async fn get_all_pool_data<M: Middleware>(
        &self,
        pools: &mut [Pool],
        request_throttle: Arc<Mutex<RequestThrottle>>,
        progress_bar: ProgressBar,
        middleware: Arc<M>,
    ) -> Result<(), CFMMError<M>> {
        match self {
            Dex::UniswapV2(_) => {
                let step = 127; //Max batch size for call
                for pools in pools.chunks_mut(step) {
                    request_throttle
                        .lock()
                        .expect("Error when acquiring request throttle mutex lock")
                        .increment_or_sleep(1);

                    batch_requests::uniswap_v2::get_pool_data_batch_request(
                        pools,
                        middleware.clone(),
                    )
                    .await?;

                    progress_bar.inc(step as u64);
                }
            }

            Dex::UniswapV3(_) => {
                let step = 76; //Max batch size for call
                for pools in pools.chunks_mut(step) {
                    request_throttle
                        .lock()
                        .expect("Error when acquiring request throttle mutex lock")
                        .increment_or_sleep(1);

                    batch_requests::uniswap_v3::get_pool_data_batch_request(
                        pools,
                        middleware.clone(),
                    )
                    .await?;

                    progress_bar.inc(step as u64);
                }
            }
        }

        //For each pair in the pairs vec, get the pool data
        Ok(())
    }

    pub fn new_empty_pool_from_event<M: Middleware>(&self, log: Log) -> Result<Pool, CFMMError<M>> {
        match self {
            Dex::UniswapV2(uniswap_v2_dex) => uniswap_v2_dex.new_empty_pool_from_event(log),
            Dex::UniswapV3(uniswap_v3_dex) => uniswap_v3_dex.new_empty_pool_from_event(log),
        }
    }

    //TODO: rename this to be specific to what it needs to do
    //This should get the pool with the best liquidity from the dex variant.
    //If univ2, there will only be one pool, if univ3 there will be multiple
    pub async fn get_pool_with_best_liquidity<M: Middleware>(
        &self,
        token_a: H160,
        token_b: H160,
        middleware: Arc<M>,
    ) -> Result<Option<Pool>, CFMMError<M>> {
        match self {
            Dex::UniswapV2(uniswap_v2_dex) => {
                let uniswap_v2_factory =
                    abi::IUniswapV2Factory::new(uniswap_v2_dex.factory_address, middleware.clone());

                let pair_address = uniswap_v2_factory.get_pair(token_a, token_b).call().await?;

                if pair_address.is_zero() {
                    Ok(None)
                } else {
                    Ok(Some(Pool::UniswapV2(
                        UniswapV2Pool::new_from_address(pair_address, middleware).await?,
                    )))
                }
            }

            Dex::UniswapV3(uniswap_v3_dex) => {
                let uniswap_v3_factory =
                    abi::IUniswapV3Factory::new(uniswap_v3_dex.factory_address, middleware.clone());

                let mut best_liquidity = 0;
                let mut best_pool_address = H160::zero();

                for fee in [100, 300, 500, 1000] {
                    let pool_address = match uniswap_v3_factory
                        .get_pool(token_a, token_b, fee)
                        .call()
                        .await
                    {
                        Ok(address) => {
                            if !address.is_zero() {
                                address
                            } else {
                                continue;
                            }
                        }
                        Err(_) => {
                            //TODO: return descriptive errors if there is an issue with the contract or if the pair does not exist
                            continue;
                        }
                    };

                    let uniswap_v3_pool =
                        abi::IUniswapV3Pool::new(pool_address, middleware.clone());

                    let liquidity = uniswap_v3_pool.liquidity().call().await?;
                    if best_liquidity < liquidity {
                        best_liquidity = liquidity;
                        best_pool_address = pool_address;
                    }
                }

                if best_pool_address.is_zero() {
                    Ok(None)
                } else {
                    Ok(Some(Pool::UniswapV3(
                        UniswapV3Pool::new_from_address(best_pool_address, middleware).await?,
                    )))
                }
            }
        }
    }

    //If univ2, there will only be one pool, if univ3 there will be multiple
    pub async fn get_all_pools_for_pair<M: Middleware>(
        &self,
        token_a: H160,
        token_b: H160,
        middleware: Arc<M>,
    ) -> Result<Option<Vec<Pool>>, CFMMError<M>> {
        match self {
            Dex::UniswapV2(uniswap_v2_dex) => {
                let uniswap_v2_factory =
                    abi::IUniswapV2Factory::new(uniswap_v2_dex.factory_address, middleware.clone());

                let pair_address = uniswap_v2_factory.get_pair(token_a, token_b).call().await?;

                if pair_address.is_zero() {
                    Ok(None)
                } else {
                    Ok(Some(vec![Pool::UniswapV2(
                        UniswapV2Pool::new_from_address(pair_address, middleware).await?,
                    )]))
                }
            }

            Dex::UniswapV3(uniswap_v3_dex) => {
                let uniswap_v3_factory =
                    abi::IUniswapV3Factory::new(uniswap_v3_dex.factory_address, middleware.clone());

                let mut pools = vec![];

                for fee in [100, 300, 500, 1000] {
                    match uniswap_v3_factory
                        .get_pool(token_a, token_b, fee)
                        .call()
                        .await
                    {
                        Ok(address) => {
                            if !address.is_zero() {
                                pools.push(Pool::UniswapV3(
                                    UniswapV3Pool::new_from_address(address, middleware.clone())
                                        .await?,
                                ))
                            }
                        }

                        Err(_) => {
                            //TODO: return descriptive errors if there is an issue with the contract or if the pair does not exist
                            continue;
                        }
                    };
                }

                if pools.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(pools))
                }
            }
        }
    }

    //Function to get all pair created events for a given Dex factory address and sync pool data
    pub async fn get_all_pools_from_logs<M: 'static + Middleware>(
        self,
        current_block: BlockNumber,
        step: usize,
        request_throttle: Arc<Mutex<RequestThrottle>>,
        progress_bar: ProgressBar,
        middleware: Arc<M>,
    ) -> Result<Vec<Pool>, CFMMError<M>> {
        //Unwrap can be used here because the creation block was verified within `Dex::new()`
        let from_block = self
            .creation_block()
            .as_number()
            .expect("Error converting creation block as number")
            .as_u64();
        let current_block = current_block
            .as_number()
            .expect("Error converting current block as number")
            .as_u64();

        let mut aggregated_pairs: Vec<Pool> = vec![];

        //Initialize the progress bar message
        progress_bar.set_length(current_block - from_block);

        //For each block within the range, get all pairs asynchronously
        for from_block in (from_block..=current_block).step_by(step) {
            let request_throttle = request_throttle.clone();
            let provider = middleware.clone();
            let progress_bar = progress_bar.clone();

            //Get pair created event logs within the block range
            let to_block = from_block + step as u64;

            //Update the throttle
            request_throttle
                .lock()
                .expect("Error when acquiring request throttle mutex lock")
                .increment_or_sleep(1);

            let logs = provider
                .get_logs(
                    &Filter::new()
                        .topic0(ValueOrArray::Value(self.pool_created_event_signature()))
                        .address(self.factory_address())
                        .from_block(BlockNumber::Number(U64([from_block])))
                        .to_block(BlockNumber::Number(U64([to_block]))),
                )
                .await
                .map_err(CFMMError::MiddlewareError)?;

            //For each pair created log, create a new Pair type and add it to the pairs vec
            for log in logs {
                let pool = self.new_empty_pool_from_event(log)?;
                aggregated_pairs.push(pool);
            }

            //Increment the progress bar by the step
            progress_bar.inc(step as u64);
        }

        Ok(aggregated_pairs)
    }

    //Function to get all pair created events for a given Dex factory address and sync pool data
    pub async fn get_all_pools_from_logs_within_range<M: 'static + Middleware>(
        self,
        from_block: BlockNumber,
        to_block: BlockNumber,
        step: usize,
        request_throttle: Arc<Mutex<RequestThrottle>>,
        progress_bar: ProgressBar,
        middleware: Arc<M>,
    ) -> Result<Vec<Pool>, CFMMError<M>> {
        //Unwrap can be used here because the creation block was verified within `Dex::new()`
        let from_block = from_block
            .as_number()
            .expect("Error converting creation block as number")
            .as_u64();
        let to_block = to_block
            .as_number()
            .expect("Error converting current block as number")
            .as_u64();

        let mut aggregated_pairs: Vec<Pool> = vec![];

        //Initialize the progress bar message
        progress_bar.set_length(to_block - from_block);

        //For each block within the range, get all pairs asynchronously
        for from_block in (from_block..=to_block).step_by(step) {
            let request_throttle = request_throttle.clone();
            let provider = middleware.clone();
            let progress_bar = progress_bar.clone();

            //Get pair created event logs within the block range
            let to_block = from_block + step as u64;

            //Update the throttle
            request_throttle
                .lock()
                .expect("Error when acquiring request throttle mutex lock")
                .increment_or_sleep(1);

            let logs = provider
                .get_logs(
                    &Filter::new()
                        .topic0(ValueOrArray::Value(self.pool_created_event_signature()))
                        .address(self.factory_address())
                        .from_block(BlockNumber::Number(U64([from_block])))
                        .to_block(BlockNumber::Number(U64([to_block]))),
                )
                .await
                .map_err(CFMMError::MiddlewareError)?;

            //For each pair created log, create a new Pair type and add it to the pairs vec
            for log in logs {
                let pool = self.new_empty_pool_from_event(log)?;
                aggregated_pairs.push(pool);
            }

            //Increment the progress bar by the step
            progress_bar.inc(step as u64);
        }

        Ok(aggregated_pairs)
    }
}

pub enum DexVariant {
    UniswapV2,
    UniswapV3,
}
impl DexVariant {
    pub fn pool_created_event_signature(&self) -> H256 {
        match self {
            DexVariant::UniswapV2 => uniswap_v2::PAIR_CREATED_EVENT_SIGNATURE,
            DexVariant::UniswapV3 => uniswap_v3::POOL_CREATED_EVENT_SIGNATURE,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env, str::FromStr, sync::Arc};

    use ethers::{
        providers::{Http, Provider},
        types::H160,
    };

    use super::{Dex, DexVariant};

    #[test]
    fn test_factory_address() {}

    #[test]
    fn test_get_pool_with_best_liquidity() {}

    #[tokio::test]
    async fn test_get_all_pools_for_pair() {
        //Univ3 on ethereum
        let univ3_pool = Dex::new(
            H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),
            DexVariant::UniswapV3,
            12369621,
            None,
        );

        let usdc = H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
        let weth = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();

        let provider = Arc::new(
            Provider::<Http>::try_from(
                env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not initialize provider"),
            )
            .unwrap(),
        );

        let pools = univ3_pool
            .get_all_pools_for_pair(usdc, weth, provider)
            .await
            .expect("Could not get all pools for pair");

        println!("Pools: {pools:?}");
    }
}
