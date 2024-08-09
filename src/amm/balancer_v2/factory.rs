use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BalancerV2Factory {
    pub address: Address,
    pub creation_block: u64,
}
