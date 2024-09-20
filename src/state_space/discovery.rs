use std::collections::HashSet;

use alloy::rpc::types::FilterSet;

use crate::amms::factory::{AutomatedMarketMakerFactory, Factory};

use super::filters::Filter;

#[derive(Debug, Default)]
pub struct DiscoveryManager {
    pub factories: Vec<Factory>,
    pub filters: Option<Vec<Filter>>,
}

impl DiscoveryManager {
    pub fn new(factories: Vec<Factory>) -> Self {
        Self {
            factories,
            ..Default::default()
        }
    }

    pub fn with_filters(self, filters: Vec<Filter>) -> Self {
        let filters = Some(filters);
        Self { filters, ..self }
    }
}
