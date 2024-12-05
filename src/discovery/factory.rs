use std::collections::HashMap;

use futures::stream::{FuturesUnordered, StreamExt};

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
        balancer_v2::factory::IBFactory, factory::Factory, uniswap_v2::factory::IUniswapV2Factory,
        uniswap_v3::factory::IUniswapV3Factory,
    },
    errors::AMMError,
};

pub enum DiscoverableFactory {
    UniswapV2Factory,
    UniswapV3Factory,
    BalancerV2Factory,
}

impl DiscoverableFactory {
    pub fn discovery_event_signature(&self) -> B256 {
        match self {
            DiscoverableFactory::UniswapV2Factory => IUniswapV2Factory::PairCreated::SIGNATURE_HASH,
            DiscoverableFactory::UniswapV3Factory => IUniswapV3Factory::PoolCreated::SIGNATURE_HASH,
            DiscoverableFactory::BalancerV2Factory => IBFactory::LOG_NEW_POOL::SIGNATURE_HASH,
        }
    }
}

// Returns a vec of empty factories that match one of the Factory interfaces specified by each DiscoverableFactory
pub async fn discover_factories<T, N, P>(
    factories: Vec<DiscoverableFactory>,
    number_of_amms_threshold: u64,
    provider: P,
    block_step: u64,
) -> Result<Vec<Factory>, AMMError>
where
    T: Transport + Clone + 'static,
    N: Network + 'static,
    P: Provider<T, N> + Clone + Send + Sync + 'static,
{
    let mut event_signatures = vec![];

    for factory in factories {
        event_signatures.push(factory.discovery_event_signature());
    }
    tracing::trace!(?event_signatures);

    // For each block within the range, get all pairs asynchronously
    let mut from_block = 0;
    let block_number = provider.get_block_number().await?;

    // set up a vector with the block range for each batch
    let mut block_num_vec: Vec<(u64, u64)> = Vec::new();

    // populate the vector
    while from_block < block_number {
        // Get pair created event logs within the block range
        let mut target_block = from_block + block_step - 1;
        if target_block > block_number {
            target_block = block_number;
        }

        block_num_vec.push((from_block, target_block));

        from_block += block_step;
    }

    // Create futures unordered
    let factories_tasks = FuturesUnordered::new();
    // Set up filter and events to filter each block you are searching by
    let block_filter = Filter::new().event_signature(event_signatures);

    // Push task to futures unordered
    for (from_block, target_block) in block_num_vec {
        let block_filter = block_filter.clone();
        let provider = provider.clone();
        factories_tasks.push(async move {
            process_block_logs_batch(&from_block, &target_block, provider, &block_filter).await
        });
    }

    // collect the results when they are finished
    let factory_results = factories_tasks.collect::<Vec<_>>().await;

    // process resulst
    let mut identified_factories: HashMap<Address, (Factory, u64)> = HashMap::new();
    for result in factory_results {
        match result {
            Ok(local_identified_factories) => {
                for (addrs, (factory, count)) in local_identified_factories {
                    identified_factories
                        .entry(addrs)
                        .and_modify(|entry| entry.1 += count) // Increment the count if the address exists
                        .or_insert((factory, count)); // Insert new entry if the address doesn't exist
                }
            }
            Err(e) => {
                // The task itself failed (possibly panicked).
                tracing::error!("Task error: {:?}", e)
            }
        }
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

async fn process_block_logs_batch<T, N, P>(
    from_block: &u64,
    target_block: &u64,
    provider: P,
    block_filter: &Filter,
) -> Result<HashMap<Address, (Factory, u64)>, AMMError>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Clone,
{
    let block_filter = block_filter.clone();
    let mut local_identified_factories: HashMap<Address, (Factory, u64)> = HashMap::new();

    let logs = provider
        .get_logs(&block_filter.from_block(*from_block).to_block(*target_block))
        .await?;

    for log in logs {
        if let Some((_, amms_length)) = local_identified_factories.get_mut(&log.address()) {
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
                Factory::BalancerV2Factory(balancer_v2_factory) => {
                    balancer_v2_factory.address = log.address();
                    balancer_v2_factory.creation_block =
                        log.block_number.ok_or(AMMError::BlockNumberNotFound)?;
                }
            }

            local_identified_factories.insert(log.address(), (factory, 0));
        }
    }

    Ok(local_identified_factories)
}
