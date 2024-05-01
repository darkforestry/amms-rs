use std::{collections::HashMap, sync::Arc};

use alloy::{
    network::Network,
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::eth::Filter,
    sol_types::SolEvent,
    transports::Transport,
};

use crate::{
    amm::{
        factory::Factory, uniswap_v2::factory::IUniswapV2Factory,
        uniswap_v3::factory::IUniswapV3Factory,
    },
    errors::AMMError,
};

pub enum DiscoverableFactory {
    UniswapV2Factory,
    UniswapV3Factory,
}

impl DiscoverableFactory {
    pub fn discovery_event_signature(&self) -> B256 {
        match self {
            DiscoverableFactory::UniswapV2Factory => IUniswapV2Factory::PairCreated::SIGNATURE_HASH,
            DiscoverableFactory::UniswapV3Factory => IUniswapV3Factory::PoolCreated::SIGNATURE_HASH,
        }
    }
}

// Returns a vec of empty factories that match one of the Factory interfaces specified by each DiscoverableFactory
pub async fn discover_factories<T, N, P>(
    factories: Vec<DiscoverableFactory>,
    number_of_amms_threshold: u64,
    provider: Arc<P>,
    step: u64,
) -> Result<Vec<Factory>, AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    let mut event_signatures = vec![];

    for factory in factories {
        event_signatures.push(factory.discovery_event_signature());
    }
    tracing::trace!(?event_signatures);

    let block_filter = Filter::new().event_signature(event_signatures);

    let mut from_block = 0;
    let current_block = provider.get_block_number().await?;

    // For each block within the range, get all pairs asynchronously
    // let step = 100000;

    // Set up filter and events to filter each block you are searching by
    let mut identified_factories: HashMap<Address, (Factory, u64)> = HashMap::new();

    // TODO: make this async
    while from_block < current_block {
        // Get pair created event logs within the block range
        let mut target_block = from_block + step - 1;
        if target_block > current_block {
            target_block = current_block;
        }

        let block_filter = block_filter.clone();
        let logs = provider
            .get_logs(&block_filter.from_block(from_block).to_block(target_block))
            .await?;

        for log in logs {
            tracing::trace!("found matching event at factory {}", log.address());
            if let Some((_, amms_length)) = identified_factories.get_mut(&log.address()) {
                *amms_length += 1;
            } else {
                let mut factory = Factory::try_from(log.topics()[0])?;

                match &mut factory {
                    Factory::UniswapV2Factory(uniswap_v2_factory) => {
                        uniswap_v2_factory.address = log.address();
                        uniswap_v2_factory.creation_block =
                            log.block_number.ok_or(AMMError::BlockNumberNotFound)?;
                    }
                    Factory::UniswapV3Factory(uniswap_v3_factory) => {
                        uniswap_v3_factory.address = log.address();
                        uniswap_v3_factory.creation_block =
                            log.block_number.ok_or(AMMError::BlockNumberNotFound)?;
                    }
                }

                identified_factories.insert(log.address(), (factory, 0));
            }
        }

        from_block += step;
    }

    let mut filtered_factories = vec![];
    tracing::trace!(number_of_amms_threshold, "checking threshold");
    for (address, (factory, amms_length)) in identified_factories {
        if amms_length >= number_of_amms_threshold {
            tracing::trace!("factory {} has {} AMMs => adding", address, amms_length);
            filtered_factories.push(factory);
        } else {
            tracing::trace!("factory {} has {} AMMs => skipping", address, amms_length);
        }
    }

    Ok(filtered_factories)
}
