use std::{error::Error, str::FromStr, sync::Arc};

use ethers::{
    providers::{Http, Provider},
    types::{H160, U256},
};

use amms::{
    amm::{
        factory::Factory,
        uniswap_v2::{factory::UniswapV2Factory, UniswapV2Pool},
        uniswap_v3::factory::UniswapV3Factory,
        AMM,
    },
    filters, sync,
};

#[tokio::main]

async fn main() -> Result<(), Box<dyn Error>> {
    //Add rpc endpoint here:
    let rpc_endpoint =
        std::env::var("POLYGON_MAINNET_ENDPOINT").expect("Could not get POLYGON_MAINNET_ENDPOINT");
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

    let factories = vec![
        //Quickswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0x5757371414417b8C6CAad45bAeF941aBc7d3Ab32").unwrap(),
            4931780,
            300,
        )),
        // Add Sushiswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0xc35DADB65012eC5796536bD9864eD8773aBc74C4").unwrap(),
            11333218,
            300,
        )),
        //Add apeswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0xCf083Be4164828f00cAE704EC15a36D711491284").unwrap(),
            15298801,
            300,
        )),
        //Add uniswap v3
        Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),
            22757547,
        )),
    ];

    //Sync pools
    let (pools, _synced_block) =
        sync::sync_amms(factories.clone(), provider.clone(), None, 10000).await?;

    //Create a list of blacklisted tokens
    let blacklisted_tokens =
        vec![H160::from_str("0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984").unwrap()];

    //Filter out blacklisted tokens
    let filtered_amms = filters::address::filter_blacklisted_tokens(pools, blacklisted_tokens);

    let weth_address = H160::from_str("0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270").unwrap();
    let usd_weth_pair_address =
        H160::from_str("0xcd353F79d9FADe311fC3119B841e1f456b54e858").unwrap();

    let usd_weth_pool = AMM::UniswapV2Pool(
        UniswapV2Pool::new_from_address(usd_weth_pair_address, 300, provider.clone()).await?,
    );

    let weth_value_in_token_to_weth_pool_threshold =
        U256::from_dec_str("50000000000000000000").unwrap(); // 5000 matic

    println!("Filtering pools below usd threshold");

    let filtered_amms = filters::value::filter_amms_below_usd_threshold(
        filtered_amms,
        &factories,
        usd_weth_pool,
        15000.00, //Setting usd_threshold to 10000.00 filters out any pool that contains less than $1m USD value
        weth_address,
        // When getting token to weth price to determine weth value in pool, dont use price with weth reserves with less than $1000 USD worth
        weth_value_in_token_to_weth_pool_threshold,
        200,
        provider.clone(),
    )
    .await?;

    println!("{:?}", filtered_amms);

    Ok(())
}

pub const U256_2_POW_18: U256 = U256([2000000000000000000, 0, 0, 0]);
