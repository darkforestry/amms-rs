use alloy::primitives::Address;

use crate::amms::amm::{AutomatedMarketMaker, AMM};

use super::filter::AMMFilter;

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

impl AMMFilter for BlacklistFilter {
    /// Filter for any AMMs or tokens not in the blacklist
    fn filter(&self, amms: Vec<AMM>) -> Vec<AMM> {
        amms.into_iter()
            .filter(|amm| {
                !self.blacklist.contains(&amm.address())
                    && amm.tokens().iter().all(|t| !self.blacklist.contains(t))
            })
            .collect()
    }
}
