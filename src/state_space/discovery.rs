use alloy::eips::BlockId;
use alloy::rpc::types::{Filter, FilterSet};
use alloy::signers::k256::elliptic_curve::rand_core::block;
use alloy::{contract::ContractInstance, sol_types::SolCall};
use alloy::{
    network::Network,
    primitives::{Address, FixedBytes},
    providers::Provider,
    rpc::types::Log,
    sol_types::{SolEvent, SolInterface},
    transports::Transport,
};
use alloy::{rpc::types::serde_helpers::quantity::vec, sol_types::JsonAbiExt};
use futures::stream::{FuturesUnordered, StreamExt};
use heimdall_decompiler::DecompilerArgsBuilder;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::amms::{
    factory::Factory,
    uniswap_v2::{
        IUniswapV2Factory::{self, IUniswapV2FactoryCalls, IUniswapV2FactoryInstance},
        UniswapV2Factory,
    },
    uniswap_v3::IUniswapV3Factory,
};

use super::filters::PoolFilter;

#[derive(Debug, Default, Clone)]
pub struct DiscoveryManager {
    pub targets: HashMap<FixedBytes<32>, DiscoverableFactory>,
    pub discovered_factories: HashMap<Address, Factory>,
    pub pool_filters: Option<Vec<PoolFilter>>,
    pub token_decimals: HashMap<Address, u8>,
}

impl DiscoveryManager {
    pub fn new(targets: Vec<DiscoverableFactory>) -> Self {
        let targets = targets
            .into_iter()
            .map(|factory| (factory.discovery_event(), factory))
            .collect();

        Self {
            targets,
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
        self.targets
            .iter()
            .fold(HashSet::new(), |mut events_set, (disc_event, _)| {
                events_set.insert(*disc_event);
                events_set
            })
    }

    pub async fn discover_factories<N, P>(&mut self, from: BlockId, to: BlockId, provider: P)
    where
        N: Network,
        P: Provider<N> + Clone,
    {
        let mut latest_block = from.as_u64().unwrap_or_default();
        let disc_filter = Filter::new().event_signature(FilterSet::from(
            self.disc_events()
                .into_iter()
                .collect::<Vec<FixedBytes<32>>>(),
        ));

        let mut futures = FuturesUnordered::new();

        let sync_step = 100_000;
        while latest_block < to.as_u64().unwrap_or_default() {
            let from_block = latest_block;
            let to_block = (from_block + sync_step).min(to.as_u64().unwrap_or_default());
            let block_filter = disc_filter
                .clone()
                .from_block(from_block)
                .to_block(to_block);

            let disc_provider = provider.clone();
            futures.push(async move { disc_provider.get_logs(&block_filter).await });
            latest_block = to_block + 1;
        }

        while let Some(res) = futures.next().await {
            let logs = res.expect("TODO: handle error");

            for log in logs {
                let Some(sig) = log.topic0() else { todo!() };

                if let Some(target) = self.targets.get(sig) {
                    let factory = target.create_factory(&log, provider.clone()).await;
                    self.discovered_factories.insert(factory.address(), factory);
                }
            }
            todo!()
        }

        todo!()
    }
}

#[derive(Clone, Debug)]
pub enum DiscoverableFactory {
    UniswapV2,
    UniswapV3,
}

impl DiscoverableFactory {
    pub fn discovery_event(&self) -> FixedBytes<32> {
        match self {
            DiscoverableFactory::UniswapV2 => IUniswapV2Factory::PairCreated::SIGNATURE_HASH,
            DiscoverableFactory::UniswapV3 => IUniswapV3Factory::PoolCreated::SIGNATURE_HASH,
        }
    }

    pub fn functions(&self) -> Vec<&'static str> {
        match self {
            DiscoverableFactory::UniswapV2 => vec![
                IUniswapV2Factory::allPairsCall::SIGNATURE,
                IUniswapV2Factory::allPairsLengthCall::SIGNATURE,
                IUniswapV2Factory::createPairCall::SIGNATURE,
                IUniswapV2Factory::feeToCall::SIGNATURE,
                IUniswapV2Factory::feeToSetterCall::SIGNATURE,
                IUniswapV2Factory::getPairCall::SIGNATURE,
                IUniswapV2Factory::setFeeToCall::SIGNATURE,
                IUniswapV2Factory::setFeeToSetterCall::SIGNATURE,
            ],
            DiscoverableFactory::UniswapV3 => todo!(),
        }
    }

    pub fn events(&self) -> Vec<&'static str> {
        match self {
            DiscoverableFactory::UniswapV2 => vec![IUniswapV2Factory::PairCreated::SIGNATURE],
            DiscoverableFactory::UniswapV3 => todo!(),
        }
    }

    pub fn errors(&self) -> Vec<&'static str> {
        match self {
            DiscoverableFactory::UniswapV2 => vec![],
            DiscoverableFactory::UniswapV3 => todo!(),
        }
    }

    // TODO: return a result
    // TODO: match on event sigs, function sigs, error sigs
    pub async fn create_factory<N, P>(&self, log: &Log, provider: P) -> Factory
    where
        N: Network,
        P: Provider<N>,
    {
        let Some(signature) = log.topic0() else {
            todo!("return error")
        };

        if *signature == self.discovery_event() {
            match self {
                DiscoverableFactory::UniswapV2 => {
                    let decompiler = DecompilerArgsBuilder::new()
                        // TODO: can we pass an addr instead?
                        .target(log.address().to_string())
                        // TODO: can we update this to use a provider?
                        .rpc_url("TODO: get endpoint from provider".to_string())
                        .build()
                        .expect("TODO: handle this error");

                    let decompiled_abi = heimdall_decompiler::decompile(decompiler)
                        .await
                        .expect("TODO: handle this error")
                        .abi;

                    // Check functions exist in decompiled abi
                    if !self
                        .functions()
                        .iter()
                        .all(|value| decompiled_abi.functions.contains_key(&value.to_string()))
                    {
                        todo!("Return error")
                    }

                    // Check events exist in decompiled abi
                    if !self
                        .events()
                        .iter()
                        .all(|value| decompiled_abi.events.contains_key(&value.to_string()))
                    {
                        todo!("Return error")
                    }

                    // TODO: dynamically get fee
                    UniswapV2Factory::new(
                        log.address(),
                        0,
                        log.block_number.expect("TODO: handle this"),
                    )
                    .into()
                }

                DiscoverableFactory::UniswapV3 => {
                    todo!()
                }
            }
        } else {
            todo!("return error");
        }
    }
}

// TODO: impl hash, use signature hash for factory
// TODO: get the factory created log from the discovery manager
// TODO: basically let factory = map.get(sig).create_factory(log,provider);
