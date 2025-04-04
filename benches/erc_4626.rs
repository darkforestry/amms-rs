use alloy::primitives::{address, U256};
use amms::amms::{amm::AutomatedMarketMaker, erc_4626::ERC4626Vault};
use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;

fn simulate_swap(c: &mut Criterion) {
    let token_a = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let token_b = address!("fc0d6cf33e38bce7ca7d89c0e292274031b7157a");

    let pool = ERC4626Vault {
        vault_token: token_a,
        vault_token_decimals: 18,
        asset_token: token_b,
        asset_token_decimals: 18,
        vault_reserve: U256::from(20_000_000_u128),
        asset_reserve: U256::from(20_000_000_u128),
        deposit_fee: 300,
        withdraw_fee: 300,
    };

    let mut rng = rand::thread_rng();
    c.bench_function("erc4626_simulate_swap", |b| {
        b.iter_with_setup(
            || U256::from(rng.gen_range(1_000..=1e24 as u128)),
            |amount| {
                let _ = pool.simulate_swap(token_a, token_b, amount).unwrap();
            },
        );
    });
}

criterion_group!(erc_4626, simulate_swap);
criterion_main!(erc_4626);
