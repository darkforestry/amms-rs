use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("add state changes benchmark", |b| {
        b.iter(|| bench_funcs::test_add_state_changes(black_box(3)));
    });
    c.bench_function("add empty state changes benchmark", |b| {
        b.iter(|| bench_funcs::test_add_empty_state_changes(black_box(3)));
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

mod bench_funcs {
    use std::sync::Arc;

    use alloy::primitives::Address;
    use amms::{
        amm::{uniswap_v2::UniswapV2Pool, AMM},
        state_space::{error::StateChangeError, StateChange, StateChangeCache},
    };

    use tokio::sync::RwLock;

    /// Duplicated helper method entirely as in `state_space::tests` module
    /// Reason of duplication: to retain encapsulation
    async fn add_state_change_to_cache(
        state_change_cache: Arc<RwLock<StateChangeCache>>,
        state_change: StateChange,
    ) -> Result<(), StateChangeError> {
        let mut state_change_cache = state_change_cache.write().await;

        if state_change_cache.is_full() {
            state_change_cache.pop_back();
            state_change_cache
                .push_front(state_change)
                .map_err(|_| StateChangeError::CapacityError)?
        } else {
            state_change_cache
                .push_front(state_change)
                .map_err(|_| StateChangeError::CapacityError)?
        }
        Ok(())
    }

    /// Duplicated method test from `state_change::tests` module
    pub async fn test_add_state_changes(n: u128) -> eyre::Result<()> {
        let state_change_cache = Arc::new(RwLock::new(StateChangeCache::new()));

        for i in 0..=n {
            let new_amm = AMM::UniswapV2Pool(UniswapV2Pool {
                address: Address::ZERO,
                reserve_0: i,
                ..Default::default()
            });

            add_state_change_to_cache(
                state_change_cache.clone(),
                StateChange::new(Some(vec![new_amm]), i as u64),
            )
            .await?;
        }

        let mut state_change_cache = state_change_cache.write().await;

        if let Some(last_state_change) = state_change_cache.pop_front() {
            if let Some(state_changes) = last_state_change.state_change() {
                assert_eq!(state_changes.len(), 1);

                if let AMM::UniswapV2Pool(pool) = &state_changes[0] {
                    assert_eq!(pool.reserve_0, n);
                } else {
                    panic!("Unexpected AMM variant")
                }
            } else {
                panic!("State changes not found")
            }
        }

        Ok(())
    }

    /// Duplicated method test from `state_change::tests` module
    pub async fn test_add_empty_state_changes(n: u64) -> eyre::Result<()> {
        let last_synced_block = 0;
        let chain_head_block_number = n;

        let state_change_cache = Arc::new(RwLock::new(StateChangeCache::new()));

        for block_number in last_synced_block..=chain_head_block_number {
            add_state_change_to_cache(
                state_change_cache.clone(),
                StateChange::new(None, block_number),
            )
            .await?;
        }

        let state_change_cache_length = state_change_cache.read().await.len();
        assert_eq!(state_change_cache_length, 101);

        Ok(())
    }
}
