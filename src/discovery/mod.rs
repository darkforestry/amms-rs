pub mod interface_check;

use ethers::types::H256;

use crate::amm;

pub enum DiscoveryTarget {
    UniswapV2Factory,
    UniswapV3Factory,
}

impl DiscoveryTarget {
    pub fn discovery_event_signature(&self) -> H256 {
        match self {
            DiscoveryTarget::UniswapV2Factory => {
                amm::uniswap_v2::factory::PAIR_CREATED_EVENT_SIGNATURE
            }

            DiscoveryTarget::UniswapV3Factory => {
                amm::uniswap_v3::factory::POOL_CREATED_EVENT_SIGNATURE
            }
        }
    }
}

//TODO: implement a function that goes through all logs and checks if a potential factory address adheres to the factory interface
// defined in the interface_check mod
