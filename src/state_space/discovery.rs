use crate::amms::factory::Factory;

use super::filters::Filter;

#[derive(Debug, Default)]
pub struct DiscoveryManager {
    pub factories: Vec<Factory>,
    pub filters: Vec<Filter>,
}

impl DiscoveryManager {
    pub fn new(factories: Vec<Factory>, filters: Vec<Filter>) -> Self {
        Self { factories, filters }
    }
}
