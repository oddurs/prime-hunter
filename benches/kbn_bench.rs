use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rug::integer::IsPrime;
use rug::Integer;

fn bench_mr_test_mersenne_127(c: &mut Criterion) {
    // 2^127 - 1 (Mersenne prime M127)
    let candidate = Integer::from(1u32) << 127u32;
    let candidate = candidate - 1u32;
    c.bench_function("is_probably_prime(M127, 25)", |b| {
        b.iter(|| {
            let _ = black_box(&candidate).is_probably_prime(black_box(25));
        });
    });
}

fn bench_mr_test_mersenne_521(c: &mut Criterion) {
    // 2^521 - 1 (Mersenne prime M521)
    let candidate = Integer::from(1u32) << 521u32;
    let candidate = candidate - 1u32;
    c.bench_function("is_probably_prime(M521, 25)", |b| {
        b.iter(|| {
            let _ = black_box(&candidate).is_probably_prime(black_box(25));
        });
    });
}

fn bench_mr_test_mersenne_2203(c: &mut Criterion) {
    // 2^2203 - 1 (Mersenne prime M2203)
    let candidate = Integer::from(1u32) << 2203u32;
    let candidate = candidate - 1u32;
    c.bench_function("is_probably_prime(M2203, 25)", |b| {
        b.iter(|| {
            let _ = black_box(&candidate).is_probably_prime(black_box(25));
        });
    });
}

fn bench_mr_screened_test_prime(c: &mut Criterion) {
    let candidate = Integer::from(1u32) << 127u32;
    let candidate = candidate - 1u32;
    c.bench_function("mr_screened_test(M127, 25)", |b| {
        b.iter(|| {
            primehunt::mr_screened_test(black_box(&candidate), black_box(25));
        });
    });
}

fn bench_mr_screened_test_composite(c: &mut Criterion) {
    // Large composite: 2^127 + 1 (not prime)
    let composite = Integer::from(1u32) << 127u32;
    let composite = composite + 1u32;
    c.bench_function("mr_screened_test(2^127+1, 25)", |b| {
        b.iter(|| {
            let result = primehunt::mr_screened_test(black_box(&composite), black_box(25));
            assert_eq!(result, IsPrime::No);
        });
    });
}

criterion_group!(
    benches,
    bench_mr_test_mersenne_127,
    bench_mr_test_mersenne_521,
    bench_mr_test_mersenne_2203,
    bench_mr_screened_test_prime,
    bench_mr_screened_test_composite,
);
criterion_main!(benches);
