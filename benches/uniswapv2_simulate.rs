use alloy::primitives::{address, U256};
use criterion::{criterion_group, criterion_main, Criterion};

use amms::amm::uniswap_v2::UniswapV2Pool;
use amms::amm::AutomatedMarketMaker;

fn criterion_benchmark(c: &mut Criterion) {
    // Data is populated from UniswapV2Pool at block 14597943
    let token_a = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let pool = UniswapV2Pool {
        address: address!("ddF8390cEd9fAD414b1ca1DD4Fe14F881C2Cfa70"),
        token_a,
        token_a_decimals: 18,
        token_b: address!("fc0d6cf33e38bce7ca7d89c0e292274031b7157a"),
        token_b_decimals: 18,
        reserve_0: 281815124409715083245_u128,
        reserve_1: 631976629342354846935765_u128,
        fee: 300,
    };
    let swap_amount = U256::from(694724330990640303_u128);
    c.bench_function("uniswapv2_simuluate", |b| {
        b.iter(|| {
            let _ = pool.simulate_swap(token_a, swap_amount).unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
