use crate::amms::factory::Factory;

#[derive(Debug, Default)]
pub struct DiscoveryManager {
    pub factories: Vec<Factory>,
    // NOTE: this is the list of filters each discovered pool will go through
    // pub filters: Vec<Filter>,
}

impl DiscoveryManager {
    pub fn new(factories: Vec<Factory>) -> Self {
        Self { factories }
    }
}
