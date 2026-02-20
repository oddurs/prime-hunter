use criterion::{black_box, criterion_group, criterion_main, Criterion};
use darkreach::sieve;

fn bench_generate_primes_1m(c: &mut Criterion) {
    c.bench_function("generate_primes(1_000_000)", |b| {
        b.iter(|| sieve::generate_primes(black_box(1_000_000)));
    });
}

fn bench_pow_mod_large(c: &mut Criterion) {
    c.bench_function("pow_mod(large base, large exp)", |b| {
        b.iter(|| {
            sieve::pow_mod(
                black_box(123_456_789),
                black_box(987_654_321),
                black_box(1_000_000_007),
            )
        });
    });
}

fn bench_multiplicative_order(c: &mut Criterion) {
    c.bench_function("multiplicative_order(2, 1000003)", |b| {
        b.iter(|| sieve::multiplicative_order(black_box(2), black_box(1_000_003)));
    });
}

fn bench_bsgs_discrete_log(c: &mut Criterion) {
    // Find x such that 2^x ≡ 500 (mod 1000003)
    let p = 1_000_003u64;
    let order = sieve::multiplicative_order(2, p);
    c.bench_function("discrete_log_bsgs(2, 500, 1000003)", |b| {
        b.iter(|| sieve::discrete_log_bsgs(black_box(2), black_box(500), black_box(p), black_box(order)));
    });
}

fn bench_montgomery_pow_mod(c: &mut Criterion) {
    let p = 1_000_000_007u64;
    let ctx = sieve::MontgomeryCtx::new(p);
    let base = ctx.to_mont(123_456_789 % p);
    c.bench_function("montgomery_pow_mod(large)", |b| {
        b.iter(|| ctx.pow_mod(black_box(base), black_box(987_654_321)));
    });
}

criterion_group!(
    benches,
    bench_generate_primes_1m,
    bench_pow_mod_large,
    bench_multiplicative_order,
    bench_bsgs_discrete_log,
    bench_montgomery_pow_mod,
);
criterion_main!(benches);
