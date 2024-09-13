use crate::amms::factory::Factory;

use super::filters::Filter;

#[derive(Debug, Default)]
pub struct DiscoveryManager {
    pub factories: Vec<Factory>,
    pub filters: Vec<Filter>,
}

impl DiscoveryManager {
    pub fn new(factories: Vec<Factory>) -> Self {
        Self {
            factories,
            ..Default::default()
        }
    }

    pub fn with_filters(self, filters: Vec<Filter>) -> Self {
        Self { filters, ..self }
    }
}
