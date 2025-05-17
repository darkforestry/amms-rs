use std::sync::Arc;

use alloy::{
    eips::BlockId,
    primitives::{address, U256},
    providers::ProviderBuilder,
    rpc::client::ClientBuilder,
    transports::layers::{RetryBackoffLayer, ThrottleLayer},
};
use amms::amms::{amm::AutomatedMarketMaker, uniswap_v3::UniswapV3Pool};
use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;
use tokio::runtime::Runtime;

fn simulate_swap(c: &mut Criterion) {
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER").expect("Could not get rpc endpoint");

    let client = ClientBuilder::default()
        .layer(ThrottleLayer::new(500))
        .layer(RetryBackoffLayer::new(5, 200, 330))
        .http(rpc_endpoint.parse().unwrap());

    let provider = Arc::new(ProviderBuilder::new().connect_client(client));

    let token_a = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let token_b = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

    let runtime = Runtime::new().expect("Failed to create Tokio runtime");
    let pool = runtime.block_on(async {
        UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"))
            .init(BlockId::latest(), provider.clone())
            .await
            .expect("Could not init pool")
    });

    let mut rng = rand::thread_rng();
    c.bench_function("uniswap_v3_simulate_swap", |b| {
        b.iter_with_setup(
            || U256::from(rng.gen_range(1_000..=1e24 as u128)),
            |amount| {
                let _ = pool.simulate_swap(token_a, token_b, amount).unwrap();
            },
        );
    });
}

criterion_group!(uniswap_v3, simulate_swap);
criterion_main!(uniswap_v3);
