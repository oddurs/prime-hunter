# Server Roadmap

Backend infrastructure: work pipeline, checkpoints, distributed coordination, verification.

**Key files:** `src/{dashboard,db,checkpoint,progress,fleet,worker_client,main}.rs`

---

## Work Pipeline (cheapest tests first)

**Current:** Single-pass: sieve then test.

**Target:** Multi-stage pipeline inspired by GIMPS:
```
Stage 1: Deep sieve (eliminate ~95% of candidates)
Stage 2: Quick PRP screen (1-2 rounds Miller-Rabin)
Stage 3: Full PRP test (Proth/LLR/25-round MR)
Stage 4: Deterministic proof (Pocklington/Morrison/BLS)
```

Each stage is cheaper than the next, and most candidates are eliminated early.

---

## Checkpoint Hardening

**Current:** Single checkpoint file, saved every 60s.

**Target:**
- Multiple backup generations (3+), rotating oldest
- Integrity verification (checksum/hash in the checkpoint file)
- Separate checkpoint files per search type to avoid conflicts

**Rationale:** A crash during checkpoint write can corrupt the only copy. Multiple generations with checksums provide defense in depth. Prime95 defaults to 3 backup files.

---

## P-1 Factoring Pre-Filter

**Current:** Only sieve-based factoring.

**Target:** For large kbn candidates that survive the sieve, run Pollard's P-1 factoring before the expensive PRP test.

```rust
fn p1_prefilter(n: &Integer, b1: u64) -> Option<Integer> {
    let mut a = Integer::from(2);
    let primes = sieve::generate_primes(b1);

    // Stage 1: a = a^(lcm(1..B1)) mod N
    for &q in &primes {
        let mut pk = q;
        while pk <= b1 / q { pk *= q; }
        a = a.pow_mod(&Integer::from(pk), n).ok()?;
    }

    let g = Integer::from(&a - 1u32).gcd(n);
    if g > 1u32 && &g < n { return Some(g); }

    // Stage 2: standard continuation with prime gaps
    // ... walk primes in (B1, B2] using precomputed differences ...
    None
}
```

**Rationale:** P-1 finds factors up to ~2^80-2^120 that are missed by trial-division sieving. GIMPS eliminates a significant fraction of candidates this way. The cost is modest (one modular exponentiation with smooth exponent) compared to a full PRP test.

The `ecm` crate (v1.0.1) on crates.io provides Lenstra ECM factoring using rug as backend — can be used as a complement to P-1.

---

## Distributed Search Coordination

**Current:** Single-machine CLI tool with basic fleet registration.

**Target:** Full coordination server that assigns work ranges to multiple machines, collects results, and avoids duplicate work.

**Approach options:**
- Simple: Shared database (the existing SQLite schema would work with a networked DB)
- Medium: REST API work-unit server (extend the existing Axum dashboard)
- Full: BOINC integration (heavy but proven infrastructure)

---

## Result Verification & Proof Generation

**Current:** Results classified as "deterministic" (small numbers GMP verifies exactly) or "probabilistic" (Miller-Rabin).

**Target:** Generate verifiable primality certificates:
- **Proth/LLR certificates:** Record the witness `a` and the computation trace
- **Pocklington/Morrison certificates:** Record witnesses for each prime factor
- **PRP proofs (Pietrzak):** Generate cryptographic VDF-style proofs verifiable in 0.5% of test time

**Rationale:** Publishable results require either deterministic proofs or independently verifiable certificates. The Top 5000 primes database (t5k.org) requires proof for deterministic claims.

---

## Elite Architecture Patterns

Lessons from Prime95, y-cruncher, GIMPS, and PrimeGrid on building world-class prime hunting software.

### FFT Memory Management (Prime95/gwnum)

- **Pooled allocator**: gwnum caches up to 10 freed gwnums for instant reuse, avoiding malloc/free in hot loops.
- **128-byte SIMD alignment**: All gwnum data aligned for AVX-512 compatibility.
- **Sin/cos table sharing**: `SHARE_SINCOS_DATA` lets multiple gwnum handles share FFT twiddle factor tables.
- **Batch allocation**: `gwalloc_array()` reduces per-element overhead.
- **Large page support**: 2MB pages reduce TLB pressure.

### Multi-Threaded FFT (Prime95 Two-Pass Architecture)

Prime95 decomposes large 1D FFTs into a 2D matrix (R x C) using the six-step FFT method:
1. **Pass 1**: C-point FFTs on each of R rows (fits in L2/L3 cache)
2. **Twiddle multiply**: Apply factors (folded into IDBWT weights)
3. **Pass 2**: R-point FFTs on each of C columns

Thread synchronization uses **atomic work-stealing**: `next_block` atomic counter lets threads claim pass-1 blocks without locks.

### Error Detection Stack

| Method | Applies To | Overhead | Detection Rate |
|--------|-----------|----------|----------------|
| Gerbicz-Li error checking | PRP tests | ~0.1% | 99.999%+ |
| Jacobi symbol check | LL tests | ~0.07% (2x/day) | ~50% per check |
| FFT round-off monitoring | All FFT-based | 0% (built-in) | Catches precision loss |
| Pietrzak PRP proofs | PRP tests | ~1-5% | Cryptographic certainty |
| Hardware self-tests | All | On-demand | Known-answer verification |

### Gerbicz Error Checking

**Algorithm:**
```
Primary sequence: u(t) = a^(2^t) mod N
Checksum sequence: d(t) = product of u(i*L) for i=0..t (mod N)

Every L^2 iterations (~4M):
  Verify: d(t+1) == u(0) * d(t)^(2^L) mod N
  If mismatch: roll back to last known-good checkpoint, replay

Overhead: ~n/L extra multiplications = ~0.1% with L=2000
```

**Rationale:** GIMPS estimated 1-2% of LL tests were corrupted by hardware errors before Gerbicz checking was adopted. For tests taking days or weeks, this is essential for result integrity.

### Self-Tuning Systems

**Prime95 runtime benchmarking:**
- Runs automatic benchmarks at 5 AM / every 21 hours
- Tests all FFT implementations for each needed size
- Results stored in `gwnum.txt`, converges after ~10 samples per size
- Up to **10% performance difference** between fastest and default FFT

**Recommendation for darkreach:** Implement startup benchmarking of FFT/multiplication sizes needed for the current search range. Store results in a local cache file.

### Memory Management Patterns for Rust/rug

```rust
// BAD: allocates every iteration
for n in start..end {
    let candidate = Integer::from(k) * Integer::from(base).pow(exp);
}

// GOOD: pre-allocate and reuse
let mut candidate = Integer::with_capacity(bit_size);
for n in start..end {
    candidate.assign(k);
    candidate <<= exp;
    candidate += 1;
}
```

**Key patterns:**
- Pre-allocate all `Integer` objects before hot loops
- Use `Integer::with_capacity(bits)` to avoid reallocation
- Arena-style batch allocation for sieve survivors
- Pre-fault memory at startup to eliminate page faults during computation

### Sieve Depth Auto-Tuning

**Current:** Fixed sieve limit.

**Target:** Dynamically determine optimal sieve depth based on the crossover heuristic: stop sieving when `time_per_factor_removed > expected_primality_test_time * probability_of_next_factor`.

**Formula (from GIMPS):**
```
Continue sieving while:
  sieve_cost_per_candidate_removed < primality_test_cost * P(factor in next range)

For primes near p, P(factor) ~ 1/ln(p)
```

### NUMA-Aware Threading

For multi-socket servers:
- Use `hwloc` to discover physical topology
- Allocate memory from the NUMA node of the threads that will access it
- Pin rayon threads to specific NUMA nodes
