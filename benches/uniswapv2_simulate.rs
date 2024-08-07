use alloy::primitives::{address, U256};
use criterion::{criterion_group, criterion_main, Criterion};

use amms::amm::uniswap_v2::UniswapV2Pool;
use amms::amm::AutomatedMarketMaker;

/// Generate a random ether amount between `from` and `to`
fn random_ether(from: f32, to: f32) -> u128 {
    let random = rand::random::<f32>() * (to - from) + from;
    random as u128 * 10u128.pow(18)
}

fn criterion_benchmark(c: &mut Criterion) {
    let token_a = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");

    let mut pool = UniswapV2Pool {
        address: address!("ddF8390cEd9fAD414b1ca1DD4Fe14F881C2Cfa70"),
        token_a,
        token_a_decimals: 18,
        token_b: address!("fc0d6cf33e38bce7ca7d89c0e292274031b7157a"),
        token_b_decimals: 18,
        reserve_0: 0_u128,
        reserve_1: 0_u128,
        fee: 300,
    };
    c.bench_function("uniswapv2_simuluate", |b| {
        b.iter(|| {
            pool.reserve_0 = random_ether(1.0, 1000.0);
            pool.reserve_1 = random_ether(1.0, 1000.0);
            let swap_amount = U256::from(random_ether(1.0, 10.0));
            let _ = pool.simulate_swap(token_a, swap_amount).unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
