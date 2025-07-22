use alloy::sol_types::SolCall;
use amms::amms::uniswap_v2::IUniswapV2Factory::{self};
use heimdall_decompiler::{decompile, DecompilerArgsBuilder};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER")?;

    let uniswap_v2_args = DecompilerArgsBuilder::new()
        .target("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".to_string())
        .rpc_url(rpc_endpoint.clone())
        .build()?;

    let decompiled_abi = decompile(uniswap_v2_args).await?.abi;

    println!("Selector {:?}", IUniswapV2Factory::allPairsCall::SELECTOR);
    println!("Signature {:?}", IUniswapV2Factory::allPairsCall::SIGNATURE);

    for (k, v) in decompiled_abi.functions {
        println!("Decompiled: {:?}: {:?}", k, v);
    }

    Ok(())
}
