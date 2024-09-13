use alloy::primitives::Address;

use crate::amms::amm::{AutomatedMarketMaker, AMM};

use super::filter::AmmFilter;

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

impl AmmFilter for WhitelistFilter {
    /// Filter for any AMMs or tokens in the whitelist
    fn filter(&self, amms: Vec<AMM>) -> Vec<AMM> {
        amms.into_iter()
            .filter(|amm| {
                self.whitelist.contains(&amm.address())
                    || amm.tokens().iter().any(|t| self.whitelist.contains(t))
            })
            .collect()
    }
}
