use alloy::primitives::Address;
use async_trait::async_trait;
use eyre::Result;

use crate::amms::amm::{AutomatedMarketMaker, AMM};

use super::{AMMFilter, FilterStage};

#[derive(Debug, Clone)]
pub struct PoolWhitelistFilter {
    pools: Vec<Address>,
}

impl PoolWhitelistFilter {
    pub fn new(pools: Vec<Address>) -> Self {
        Self { pools }
    }
}

#[async_trait]
impl AMMFilter for PoolWhitelistFilter {
    /// Filter for any AMMs or tokens in the whitelist
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>> {
        Ok(amms
            .into_iter()
            .filter(|amm| self.pools.contains(&amm.address()))
            .collect())
    }

    fn stage(&self) -> FilterStage {
        FilterStage::Discovery
    }
}

#[derive(Debug, Clone)]
pub struct TokenWhitelistFilter {
    tokens: Vec<Address>,
}

impl TokenWhitelistFilter {
    pub fn new(tokens: Vec<Address>) -> Self {
        Self { tokens }
    }
}

#[async_trait]
impl AMMFilter for TokenWhitelistFilter {
    /// Filter for any AMMs or tokens in the whitelist
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>> {
        Ok(amms
            .into_iter()
            .filter(|amm| amm.tokens().iter().any(|t| self.tokens.contains(t)))
            .collect())
    }

    fn stage(&self) -> FilterStage {
        FilterStage::Sync
    }
}
