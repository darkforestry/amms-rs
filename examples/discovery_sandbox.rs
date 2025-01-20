use std::sync::Arc;

use alloy::{
    primitives::{address, Address}, providers::ProviderBuilder, rpc::client::ClientBuilder, sol,
    transports::layers::RetryBackoffLayer,
};
use alloy_throttle::ThrottleLayer;
use amms::{
    amms::{
        erc_4626::ERC4626Vault,
        uniswap_v2::{IUniswapV2Factory::{self, IUniswapV2FactoryCalls, IUniswapV2FactoryEvents, IUniswapV2FactoryInstance}, UniswapV2Factory, UniswapV2Pool},
        uniswap_v3::{UniswapV3Factory, UniswapV3Pool},
    },
    state_space::StateSpaceBuilder,
};
use heimdall_decompiler::{decompile, DecompilerArgs, DecompilerArgsBuilder};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

    // IUniswapV2FactoryCalls::SELECTORS
    // IUniswapV2FactoryEvents::SELECTORS

    let uv2_args = DecompilerArgsBuilder::new()
        // .target("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".to_string())
        // .rpc_url(rpc_endpoint.clone())
        .build()?;

    let sushi_args = DecompilerArgsBuilder::new()
        .target("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".to_string())
        .rpc_url(rpc_endpoint)
        .build()?;

    // TODO: try with json abi and then comapare the two

    let uv2 = decompile(uv2_args).await?;
    let sushi = decompile(sushi_args).await?;

    let abi = IUniswapV2FactoryInstance::new(Address::ZERO, provider)

    for (func, _) in uv2.abi.functions.iter() {
        println!("Function: {:#?}", func);
        println!("func in sushi {}", sushi.abi.functions.contains_key(func));
    }

    for (event, _) in uv2.abi.events.iter() {
        println!("Event: {:#?}", event);
        println!("event in sushi {}", sushi.abi.events.contains_key(event));
    }

    for (error, _) in uv2.abi.errors.iter() {
        println!("Error: {:#?}", error);
        println!("error in sushi {}", sushi.abi.errors.contains_key(error));
    }

    // println!("Decompiled contract: {:#?}", uv2);

    Ok(())
}
