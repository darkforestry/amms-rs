use std::sync::Arc;

use ethers::{
    providers::Middleware,
    types::{H160, H256},
};
use serde::{Deserialize, Serialize};

use crate::errors::{ArithmeticError, DAMMError};

use super::AutomatedMarketMaker;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct X {}

#[async_trait::async_trait]
impl AutomatedMarketMaker for X {
    fn address(&self) -> H160 {
        H160::zero()
    }

    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
        Ok(())
    }

    fn sync_on_event_signature(&self) -> H256 {
        H256::zero()
    }

    //Calculates base/quote, meaning the price of base token per quote (ie. exchange rate is X base per 1 quote)
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        Ok(0.0)
    }

    fn tokens(&self) -> Vec<H160> {
        vec![]
    }
}
