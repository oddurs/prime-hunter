//! Benchmarks comparing GMP vs FLINT for factorial and primorial computation.
//!
//! Run with: `cargo bench --features flint --bench flint_bench`

use criterion::{criterion_group, criterion_main, Criterion};
use rug::Integer;

fn bench_factorial(c: &mut Criterion) {
    let mut group = c.benchmark_group("factorial");

    for &n in &[1_000u64, 10_000, 100_000] {
        group.bench_function(format!("gmp_{}", n), |b| {
            b.iter(|| Integer::from(Integer::factorial(n as u32)))
        });

        #[cfg(feature = "flint")]
        group.bench_function(format!("flint_{}", n), |b| {
            b.iter(|| primehunt::flint::factorial(n))
        });
    }

    group.finish();
}

fn bench_primorial(c: &mut Criterion) {
    let mut group = c.benchmark_group("primorial");

    for &n in &[1_000u64, 10_000, 100_000] {
        group.bench_function(format!("gmp_{}", n), |b| {
            b.iter(|| Integer::from(Integer::primorial(n as u32)))
        });

        #[cfg(feature = "flint")]
        group.bench_function(format!("flint_{}", n), |b| {
            b.iter(|| primehunt::flint::primorial(n))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_factorial, bench_primorial);
criterion_main!(benches);
