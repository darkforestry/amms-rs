use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("add state changes benchmark", |b| {
        b.iter(|| amms::state_space::test_utils::test_add_state_changes(black_box(3)));
    });
    c.bench_function("add empty state changes benchmark", |b| {
        b.iter(|| amms::state_space::test_utils::test_add_empty_state_changes(black_box(3)));
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);