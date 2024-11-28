use alloy::primitives::Address;
use async_trait::async_trait;
use eyre::Result;

use crate::amms::amm::{AutomatedMarketMaker, AMM};

use super::filter::{AMMFilter, FilterStage};

#[derive(Debug, Clone)]
pub struct WhitelistFilter {
    /// A whitelist of addresses to exclusively allow
    whitelist: Vec<Address>,
}

impl WhitelistFilter {
    pub fn new(whitelist: Vec<Address>) -> Self {
        Self { whitelist }
    }
}

#[async_trait]
impl AMMFilter for WhitelistFilter {
    /// Filter for any AMMs or tokens in the whitelist
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>> {
        Ok(amms
            .into_iter()
            .filter(|amm| {
                self.whitelist.contains(&amm.address())
                    || amm.tokens().iter().any(|t| self.whitelist.contains(t))
            })
            .collect())
    }

    fn stage(&self) -> FilterStage {
        FilterStage::Discovery
    }
}
