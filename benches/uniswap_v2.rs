use alloy::primitives::{address, U256};
use amms::amms::{amm::AutomatedMarketMaker, uniswap_v2::UniswapV2Pool};
use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;

fn simulate_swap(c: &mut Criterion) {
    let token_a = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let token_b = address!("fc0d6cf33e38bce7ca7d89c0e292274031b7157a");

    let pool = UniswapV2Pool {
        token_a,
        token_a_decimals: 18,
        token_b,
        token_b_decimals: 18,
        reserve_0: 20_000_000_u128,
        reserve_1: 20_000_000_u128,
        fee: 3000,
        ..Default::default()
    };

    let mut rng = rand::thread_rng();
    c.bench_function("uniswap_v2_simulate_swap", |b| {
        b.iter_with_setup(
            || U256::from(rng.gen_range(1_000..=1e24 as u128)),
            |amount| {
                let _ = pool.simulate_swap(token_a, token_b, amount).unwrap();
            },
        );
    });
}

// TODO: bench syncing

criterion_group!(uniswap_v2, simulate_swap);
criterion_main!(uniswap_v2);
