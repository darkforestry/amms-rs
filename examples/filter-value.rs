use std::sync::Arc;

use alloy::{
    primitives::{address, U256},
    providers::ProviderBuilder,
};

use amms::{
    amm::{
        factory::Factory,
        uniswap_v2::{factory::UniswapV2Factory, UniswapV2Pool},
        AMM,
    },
    filters, sync,
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse()?));

    // Initialize factories
    let factories = vec![
        // Add UniswapV2
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            2638438,
            300,
        )),
        // Add Sushiswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            address!("C0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"),
            10794229,
            300,
        )),
    ];

    // Sync pools
    let (pools, _synced_block) =
        sync::sync_amms(factories.clone(), provider.clone(), None, 10000).await?;

    // Filter out blacklisted tokens
    let blacklisted_tokens = vec![address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984")];
    let filtered_amms = filters::address::filter_blacklisted_tokens(pools, blacklisted_tokens);

    // Filter out pools below usd threshold
    let weth_address = address!("0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270");
    let usd_weth_pair_address = address!("cd353F79d9FADe311fC3119B841e1f456b54e858");
    let usd_weth_pool = AMM::UniswapV2Pool(
        UniswapV2Pool::new_from_address(usd_weth_pair_address, 300, provider.clone()).await?,
    );
    let weth_value_in_token_to_weth_pool_threshold = U256::from(100000000000000000_u128); // 10 weth

    println!("Filtering pools below usd threshold");

    let _filtered_amms = filters::value::filter_amms_below_usd_threshold(
        filtered_amms,
        &factories,
        usd_weth_pool,
        15000.00, //Setting usd_threshold to 15000 filters out any pool that contains less than $15000.00 USD value
        weth_address,
        weth_value_in_token_to_weth_pool_threshold,
        200,
        provider.clone(),
    )
    .await?;

    Ok(())
}
