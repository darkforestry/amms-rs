use async_trait::async_trait;
use ethers::types::{H160, H256, U256};

#[async_trait]
pub trait AutomatedMarketMaker {
    fn address(&self) -> H160;
    fn calculate_price(&self) -> f64;
    fn simulate_swap(&self, token_in: H160, amount_in: U256, token_out: H160) -> U256;
    fn stimulate_swap_mut(&mut self, token_in: H160, amount_in: U256, token_out: H160) -> U256;
    fn tokens(&self) -> Vec<H160>;
    fn sync(&self);
    async fn sync_from_log(&self);
    async fn sync_events(&self) -> Vec<H256>;
    // swap_calldata //TODO: not sure if we can have this as a requirement for the the trait but we should have this on every type
}

pub enum AMM {
    UniswapV2Pool(),
    UniswapV3Pool(),
}
