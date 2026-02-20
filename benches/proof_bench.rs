use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rug::Integer;

fn bench_pocklington_proof(c: &mut Criterion) {
    let sieve_primes = darkreach::sieve::generate_primes(100);
    // 27! + 1 is prime
    let mut factorial = Integer::from(1);
    for i in 2..=27u64 {
        factorial *= i;
    }
    let candidate = Integer::from(&factorial + 1u32);

    c.bench_function("pocklington_proof(27!+1)", |b| {
        b.iter(|| {
            darkreach::proof::pocklington_factorial_proof(
                black_box(27),
                black_box(&candidate),
                black_box(&sieve_primes),
            )
        });
    });
}

fn bench_morrison_proof(c: &mut Criterion) {
    let sieve_primes = darkreach::sieve::generate_primes(100);
    // 4! - 1 = 23 is prime. Morrison works on n!-1.
    let candidate = Integer::from(23);

    c.bench_function("morrison_proof(4!-1)", |b| {
        b.iter(|| {
            darkreach::proof::morrison_factorial_proof(
                black_box(4),
                black_box(&candidate),
                black_box(&sieve_primes),
            )
        });
    });
}

fn bench_pocklington_proof_large(c: &mut Criterion) {
    let sieve_primes = darkreach::sieve::generate_primes(100);
    // 41! + 1 is prime
    let mut factorial = Integer::from(1);
    for i in 2..=41u64 {
        factorial *= i;
    }
    let candidate = Integer::from(&factorial + 1u32);

    c.bench_function("pocklington_proof(41!+1)", |b| {
        b.iter(|| {
            darkreach::proof::pocklington_factorial_proof(
                black_box(41),
                black_box(&candidate),
                black_box(&sieve_primes),
            )
        });
    });
}

criterion_group!(
    benches,
    bench_pocklington_proof,
    bench_morrison_proof,
    bench_pocklington_proof_large,
);
criterion_main!(benches);
