use amms::{
    amm::{uniswap_v2::UniswapV2Pool, AMM},
    state_space::{cache::StateChangeCache, StateChange},
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn add_state_changes_benchmark(c: &mut Criterion) {
    let state_changes: Vec<StateChange> = (0..150)
        .map(|i| {
            let amms = vec![AMM::UniswapV2Pool(UniswapV2Pool::default()); 100];

            StateChange {
                block_number: i,
                state_change: amms,
            }
        })
        .collect();

    // Benchmark adding state changes to the cache with setup
    c.bench_function("add state changes to cache with setup", |b| {
        b.iter_batched(
            || StateChangeCache::new(),
            |mut cache| {
                for state_change in state_changes.clone() {
                    cache
                        .add_state_change_to_cache(black_box(state_change))
                        .unwrap();
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, add_state_changes_benchmark);
criterion_main!(benches);
