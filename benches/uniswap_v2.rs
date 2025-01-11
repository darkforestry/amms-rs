use alloy::primitives::{address, U256};
use amms::amms::{amm::AutomatedMarketMaker, uniswap_v2::UniswapV2Pool, Token};
use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;

fn simulate_swap(c: &mut Criterion) {
    let pool = UniswapV2Pool {
        token_a: Token::new_with_decimals(address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"), 18),
        token_b: Token::new_with_decimals(address!("fc0d6cf33e38bce7ca7d89c0e292274031b7157a"), 18),
        reserve_0: 20_000_000_u128,
        reserve_1: 20_000_000_u128,
        fee: 300,
        ..Default::default()
    };

    let mut rng = rand::thread_rng();
    c.bench_function("uniswap_v2_simulate_swap", |b| {
        b.iter_with_setup(
            || U256::from(rng.gen_range(1_000..=1e24 as u128)),
            |amount| {
                let _ = pool
                    .simulate_swap(pool.token_a.address(), pool.token_b.address(), amount)
                    .unwrap();
            },
        );
    });
}

// TODO: bench syncing

criterion_group!(uniswap_v2, simulate_swap);
criterion_main!(uniswap_v2);
