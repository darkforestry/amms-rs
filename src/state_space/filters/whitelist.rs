use alloy::primitives::Address;
use async_trait::async_trait;
use eyre::Result;

use crate::amms::amm::{AutomatedMarketMaker, AMM};

use super::filter::{AMMFilter, FilterStage};

#[derive(Debug, Clone)]
pub struct WhitelistFilter {
    /// A whitelist of addresses to exclusively allow
    pools: Vec<Address>,
    tokens: Vec<Address>,
}

impl WhitelistFilter {
    pub fn new() -> Self {
        Self {
            pools: vec![],
            tokens: vec![],
        }
    }

    pub fn with_pools(mut self, pools: Vec<Address>) -> Self {
        self.pools = pools;
        self
    }

    pub fn with_tokens(mut self, tokens: Vec<Address>) -> Self {
        self.tokens = tokens;
        self
    }
}

#[async_trait]
impl AMMFilter for WhitelistFilter {
    /// Filter for any AMMs or tokens in the whitelist
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>> {
        Ok(amms
            .into_iter()
            .filter(|amm| {
                self.pools.contains(&amm.address())
                    || amm.tokens().iter().any(|t| self.tokens.contains(t))
            })
            .collect())
    }

    fn stage(&self) -> FilterStage {
        FilterStage::Discovery
    }
}
