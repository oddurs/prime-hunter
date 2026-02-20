use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rug::Integer;

fn bench_has_small_factor_prime(c: &mut Criterion) {
    // 2^127 - 1 (Mersenne prime, no small factors)
    let n = Integer::from(1u32) << 127u32;
    let prime = n - 1u32;
    c.bench_function("has_small_factor(M127)", |b| {
        b.iter(|| darkreach::has_small_factor(black_box(&prime)));
    });
}

fn bench_has_small_factor_composite(c: &mut Criterion) {
    // Large composite: 2^128 (divisible by 2)
    let composite = Integer::from(1u32) << 128u32;
    c.bench_function("has_small_factor(2^128)", |b| {
        b.iter(|| darkreach::has_small_factor(black_box(&composite)));
    });
}

fn bench_mr_screened_prime(c: &mut Criterion) {
    // 2^127 - 1 (Mersenne prime)
    let n = Integer::from(1u32) << 127u32;
    let prime = n - 1u32;
    c.bench_function("mr_screened_test(M127, 25)", |b| {
        b.iter(|| darkreach::mr_screened_test(black_box(&prime), black_box(25)));
    });
}

fn bench_mr_screened_composite(c: &mut Criterion) {
    // Large Carmichael-like composite: 561 = 3 * 11 * 17
    let composite = Integer::from(561);
    c.bench_function("mr_screened_test(561, 25)", |b| {
        b.iter(|| darkreach::mr_screened_test(black_box(&composite), black_box(25)));
    });
}

fn bench_estimate_digits(c: &mut Criterion) {
    let n = Integer::from(1u32) << 10000u32;
    c.bench_function("estimate_digits(2^10000)", |b| {
        b.iter(|| darkreach::estimate_digits(black_box(&n)));
    });
}

fn bench_checkpoint_save_load(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bench_checkpoint.json");
    let cp = darkreach::checkpoint::Checkpoint::Factorial {
        last_n: 12345,
        start: Some(1),
        end: Some(100000),
    };

    c.bench_function("checkpoint_save_load", |b| {
        b.iter(|| {
            darkreach::checkpoint::save(black_box(&path), black_box(&cp)).unwrap();
            darkreach::checkpoint::load(black_box(&path)).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_has_small_factor_prime,
    bench_has_small_factor_composite,
    bench_mr_screened_prime,
    bench_mr_screened_composite,
    bench_estimate_digits,
    bench_checkpoint_save_load,
);
criterion_main!(benches);
