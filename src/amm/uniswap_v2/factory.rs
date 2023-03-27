use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::Middleware,
    types::{BlockNumber, H160},
};

use crate::{
    amm::{factory::AutomatedMarketMakerFactory, AMM},
    errors::DAMMError,
};

use super::batch_request;

#[derive(Clone, Copy)]
pub struct UniswapV2Factory {
    pub factory_address: H160,
    pub creation_block: BlockNumber,
    pub fee: u64,
}

#[async_trait]
impl AutomatedMarketMakerFactory for UniswapV2Factory {
    async fn get_all_amms<M: Middleware>(
        &self,
        step: usize,
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        self.get_all_pairs_via_batched_calls(middleware).await
    }

    async fn get_all_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        middleware: Arc<M>,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        let step = 127; //Max batch size for call
        for amm_chunk in amms.chunks_mut(step) {
            batch_request::get_amm_data_batch_request(amms, middleware.clone()).await?;

            //TODO: add back progress bars
            // progress_bar.inc(step as u64);
        }
    }
}
