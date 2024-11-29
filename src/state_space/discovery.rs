use std::collections::{HashMap, HashSet};

use alloy::primitives::{Address, FixedBytes};

use crate::amms::factory::Factory;

use super::filters::PoolFilter;

#[derive(Debug, Default, Clone)]
pub struct DiscoveryManager {
    pub factories: HashMap<Address, Factory>,
    pub pool_filters: Option<Vec<PoolFilter>>,
    pub token_decimals: HashMap<Address, u8>,
}

impl DiscoveryManager {
    pub fn new(factories: Vec<Factory>) -> Self {
        let factories = factories
            .into_iter()
            .map(|factory| {
                let address = factory.address();
                (address, factory)
            })
            .collect();
        Self {
            factories,
            ..Default::default()
        }
    }

    pub fn with_pool_filters(self, pool_filters: Vec<PoolFilter>) -> Self {
        Self {
            pool_filters: Some(pool_filters),
            ..self
        }
    }

    pub fn disc_events(&self) -> HashSet<FixedBytes<32>> {
        self.factories
            .iter()
            .fold(HashSet::new(), |mut events_set, (_, factory)| {
                events_set.extend([factory.discovery_event()]);
                events_set
            })
    }
}
