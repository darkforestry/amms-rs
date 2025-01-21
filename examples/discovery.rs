use std::sync::Arc;

use alloy::{
    primitives::{address, Address},
    providers::ProviderBuilder,
    rpc::client::ClientBuilder,
    sol,
    sol_types::SolCall,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;
use amms::{
    amms::{
        erc_4626::ERC4626Vault,
        uniswap_v2::{
            IUniswapV2Factory::{
                self, IUniswapV2FactoryCalls, IUniswapV2FactoryEvents, IUniswapV2FactoryInstance,
            },
            UniswapV2Factory, UniswapV2Pool,
        },
        uniswap_v3::{UniswapV3Factory, UniswapV3Pool},
    },
    state_space::{discovery::DiscoverableFactory, StateSpaceBuilder},
};
use heimdall_decompiler::{decompile, DecompilerArgs, DecompilerArgsBuilder};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

    let uniswap_v2_args = DecompilerArgsBuilder::new()
        .target("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".to_string())
        .rpc_url(rpc_endpoint.clone())
        .build()?;

    let decompiled_abi = decompile(uniswap_v2_args).await?.abi;

    for (k, v) in decompiled_abi.functions {
        println!("v: {:?}", v);
    }

    let factory = DiscoverableFactory::UniswapV2;
    for function in factory.functions() {
        println!("Function: {}", function);
    }

    // // Check functions exist in decompiled abi
    // if !factory
    //     .functions()
    //     .iter()
    //     .all(|value| decompiled_abi.functions.contains_key(&value.to_string()))
    // {
    //     todo!("Return error")
    // }

    // // Check events exist in decompiled abi
    // if !factory
    //     .events()
    //     .iter()
    //     .all(|value| decompiled_abi.events.contains_key(&value.to_string()))
    // {
    //     todo!("Return error")
    // }

    Ok(())
}
