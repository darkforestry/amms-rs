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
    pub factories: HashMap<Address, Factory>,
    pub pool_filters: Option<Vec<PoolFilter>>,
    pub token_decimals: HashMap<Address, u8>,
}

// TODO: have some way to eval pools for some period of time and then drop them if they do not cleared or add them to the state space
// TODO: should also track what is already found and ignore events if already found

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

// TODO: disc event
// TODO: match on event sigs, function sigs, error sigs

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
    pub async fn create_factory<N, P>(&self, log: Log, provider: Arc<P>) -> Factory
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
