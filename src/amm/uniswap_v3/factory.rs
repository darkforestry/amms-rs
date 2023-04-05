use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    abi::ParamType,
    prelude::abigen,
    providers::Middleware,
    types::{BlockNumber, Filter, Log, ValueOrArray, H160, H256, U256, U64},
};
use serde::{Deserialize, Serialize};

use crate::{
    amm::{factory::AutomatedMarketMakerFactory, AMM},
    errors::DAMMError,
};

use super::{batch_request, UniswapV3Pool};

abigen!(
    IUniswapV3Factory,
    r#"[
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool)
        event PoolCreated(address indexed token0, address indexed token1, uint24 indexed fee, int24 tickSpacing, address pool)
        parameters() returns (address, address, uint24, int24)
        feeAmountTickSpacing(uint24) returns (int24)
        ]"#;
);

pub const POOL_CREATED_EVENT_SIGNATURE: H256 = H256([
    120, 60, 202, 28, 4, 18, 221, 13, 105, 94, 120, 69, 104, 201, 109, 162, 233, 194, 47, 249, 137,
    53, 122, 46, 139, 29, 155, 43, 78, 107, 113, 24,
]);

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UniswapV3Factory {
    pub address: H160,
    pub creation_block: u64,
}

#[async_trait]
impl AutomatedMarketMakerFactory for UniswapV3Factory {
    fn address(&self) -> H160 {
        self.address
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }

    fn amm_created_event_signature(&self) -> H256 {
        POOL_CREATED_EVENT_SIGNATURE
    }

    async fn new_amm_from_log<M: Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, DAMMError<M>> {
        let tokens = ethers::abi::decode(&[ParamType::Uint(32), ParamType::Address], &log.data)?;
        let pair_address = tokens[1].to_owned().into_address().unwrap();

        Ok(AMM::UniswapV3Pool(
            UniswapV3Pool::new_from_address(pair_address, middleware).await?,
        ))
    }

    async fn get_all_amms<M: Middleware>(
        &self,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        let current_block = middleware
            .get_block_number()
            .await
            .map_err(DAMMError::MiddlewareError)?;

        self.get_all_pools_from_logs(current_block.into(), 100000, middleware)
            .await
    }

    async fn populate_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        let step = 127; //Max batch size for call
        for amm_chunk in amms.chunks_mut(step) {
            batch_request::get_amm_data_batch_request(amm_chunk, middleware.clone()).await?;

            //TODO: add back progress bars
            // progress_bar.inc(step as u64);
        }

        Ok(())
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error> {
        let tokens = ethers::abi::decode(&[ParamType::Uint(32), ParamType::Address], &log.data)?;
        let token_a = H160::from(log.topics[0]);
        let token_b = H160::from(log.topics[1]);
        let fee = tokens[0].to_owned().into_uint().unwrap().as_u32();
        let address = tokens[1].to_owned().into_address().unwrap();

        Ok(AMM::UniswapV3Pool(UniswapV3Pool {
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
}

impl UniswapV3Factory {
    pub fn new(address: H160, creation_block: u64) -> UniswapV3Factory {
        UniswapV3Factory {
            address,
            creation_block,
        }
    }

    //Function to get all pair created events for a given Dex factory address and sync pool data
    pub async fn get_all_pools_from_logs<M: Middleware>(
        self,
        current_block: BlockNumber,
        step: usize,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        //Unwrap can be used here because the creation block was verified within `Dex::new()`
        let from_block = self.creation_block;
        let current_block = current_block
            .as_number()
            .expect("Error converting current block as number")
            .as_u64();

        let mut aggregated_amms: Vec<AMM> = vec![];

        //For each block within the range, get all pairs asynchronously
        for from_block in (from_block..=current_block).step_by(step) {
            let provider = middleware.clone();

            //Get pair created event logs within the block range
            let to_block = from_block + step as u64;

            let logs = provider
                .get_logs(
                    &Filter::new()
                        .topic0(ValueOrArray::Value(POOL_CREATED_EVENT_SIGNATURE))
                        .address(self.address)
                        .from_block(BlockNumber::Number(U64([from_block])))
                        .to_block(BlockNumber::Number(U64([to_block]))),
                )
                .await
                .map_err(DAMMError::MiddlewareError)?;

            //For each pair created log, create a new Pair type and add it to the pairs vec
            for log in logs {
                let amm = self.new_empty_amm_from_log(log)?;
                aggregated_amms.push(amm);
            }

            //Increment the progress bar by the step
            // progress_bar.inc(step as u64);
        }

        Ok(aggregated_amms)
    }
}
