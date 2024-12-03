use alloy::primitives::Address;
use async_trait::async_trait;

use crate::amms::{
    amm::{AutomatedMarketMaker, AMM},
    error::AMMError,
};

use super::{AMMFilter, FilterStage};

#[derive(Debug, Clone)]
pub struct BlacklistFilter {
    /// A blacklist of addresses to exclusively disallow
    blacklist: Vec<Address>,
}

impl BlacklistFilter {
    pub fn new(blacklist: Vec<Address>) -> Self {
        Self { blacklist }
    }
}

#[async_trait]
impl AMMFilter for BlacklistFilter {
    /// Filter for any AMMs or tokens not in the blacklist
    async fn filter(&self, amms: Vec<AMM>) -> Result<Vec<AMM>, AMMError> {
        Ok(amms
            .into_iter()
            .filter(|amm| {
                !self.blacklist.contains(&amm.address())
                    && amm.tokens().iter().all(|t| !self.blacklist.contains(t))
            })
            .collect())
    }

    /// Filter stage is set to after the sync stage to ensure the blacklist is applied to all
    /// pool and token addresses after the pools have been fully populated
    fn stage(&self) -> FilterStage {
        FilterStage::Sync
    }
}
