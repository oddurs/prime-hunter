# GWNUM & FLINT Integration

Comprehensive guide to integrating GWNUM (direct FFI and subprocess) and FLINT for accelerated primality testing and arithmetic in primehunt.

---

## Overview

At frontier candidate sizes (1.5M-3.4M digits), GWNUM provides 50-100x speedup over GMP. Without it, several prime forms are computationally infeasible:

| Form | Frontier size | GMP time/candidate | GWNUM time | Speedup |
|------|--------------|-------------------|-----------|---------|
| Factorial n!+-1 | n~700K (3.4M digits) | ~12-14 months | ~2 weeks | 50-100x |
| Wagstaff (2^p+1)/3 | p~5M (1.5M digits) | ~6 months | ~3 days | 50-100x |
| Palindromic near-repdigit | 3M digits | ~months | ~days | 50-100x |
| kbn k*b^n+-1 | n~1M (300K digits) | ~hours | ~minutes | 10-50x |

---

## Architecture Decision Records

### ADR-1: Subprocess vs Direct FFI

**Decision:** Use both — subprocess (PFGW/PRST) as the primary path, direct GWNUM FFI as the advanced path.

**Context:**
- PRST subprocess integration already exists for k*b^n+-1 forms (`src/prst.rs`)
- Factorial, palindromic, wagstaff, primorial, near_repdigit have zero acceleration
- Direct FFI enables custom squaring loops (Vrba-Reix), progress reporting, and Gerbicz error checking

**Rationale:**
| Criterion | Subprocess (PFGW/PRST) | Direct FFI (GWNUM) |
|-----------|----------------------|-------------------|
| Implementation effort | Low (~250 lines) | High (~1000 lines) |
| Performance control | None (black box) | Full (custom loops) |
| Progress reporting | Parse stderr | Native callbacks |
| Error checking | Trust subprocess | Gerbicz integration |
| Platform support | x86-64 binaries | x86-64 only (assembly) |
| Licensing | GPL (PFGW binary) | GPL (gwnum source) |

**Consequence:** Phase 0 (PFGW subprocess) unblocks all forms immediately. Phase 3 (GWNUM FFI) provides deeper integration for Wagstaff (Vrba-Reix) and enhanced kbn (custom Proth/LLR with Gerbicz).

### ADR-2: FLINT for Multiplication Acceleration

**Decision:** Use FLINT as an optional feature for fast factorial/primorial computation.

**Context:**
- GMP's multiplication is the bottleneck for factorial/primorial precomputation
- FLINT 3.4's `fft_small` uses SIMD-vectorized NTTs (AVX2 on x86, NEON on ARM)
- Unlike GWNUM, FLINT works on Apple Silicon

**Rationale:**
- 3-10x speedup for the multiply step (not the primality test)
- BSD license (no GPL concerns)
- `flint3-sys` crate exists on crates.io
- Apple Silicon support via NEON

### ADR-3: Feature Flags for Optional Dependencies

**Decision:** Gate FLINT and GWNUM behind Cargo feature flags.

```toml
[features]
default = []
flint = ["flint3-sys"]
gwnum = ["gwnum-sys"]
```

**Rationale:** Neither FLINT nor GWNUM should be required to build primehunt. CI runs tests with and without features. Users on ARM (Apple Silicon) can use FLINT but not GWNUM.

---

## Integration Decision Tree

For each candidate, the testing pipeline follows this priority order:

```
Candidate arrives
  |
  v
Is it k*b^n+-1 form?
  |-- Yes --> GWNUM direct (if --features gwnum)
  |           --> PRST subprocess (if prst binary found)
  |               --> GMP Proth/LLR/MR (fallback)
  |
  |-- No --> Is digit count >= pfgw_min_digits?
              |-- Yes --> PFGW subprocess
              |           --> GMP MR (fallback)
              |-- No --> GMP MR directly
```

### Form-specific tool selection

| Form | Best tool | Expression format | Test type |
|------|-----------|------------------|-----------|
| kbn k*b^n+1 | GWNUM gwnum_proth / PRST | `ABC $a*$b^$c+1` | Proth (deterministic) |
| kbn k*b^n-1 | GWNUM gwnum_llr / PRST | `ABC $a*$b^$c-1` | LLR (deterministic) |
| Factorial n!+1 | PFGW -tp | `n!+1` | N-1 proof (deterministic) |
| Factorial n!-1 | PFGW -tm | `n!-1` | N+1 proof (deterministic) |
| Primorial p#+1 | PFGW -tp | `p#+1` | N-1 proof (deterministic) |
| Primorial p#-1 | PFGW -tm | `p#-1` | N+1 proof (deterministic) |
| Wagstaff (2^p+1)/3 | GWNUM vrba_reix / PFGW | `(2^p+1)/3` | PRP only (no proof exists) |
| Palindromic | PFGW | decimal digit string | PRP |
| Near-repdigit | PFGW -tp | algebraic expression | N-1 proof possible |
| Cullen n*2^n+1 | PRST (via kbn) | `ABC $a*2^$c+1` | Proth |
| Woodall n*2^n-1 | PRST (via kbn) | `ABC $a*2^$c-1` | LLR |
| Twin | PRST (via kbn) | k*b^n+-1 pair | Proth+LLR |
| Sophie Germain | PRST (via kbn) | k*b^n-1 pair | LLR |
| Carol/Kynea | PRST (via kbn) | k*b^n+-1 form | Proth/LLR |
| Repunit | PFGW | `(b^n-1)/(b-1)` | PRP |
| Gen Fermat | PRST (via kbn) | `b^(2^n)+1` | Proth |

---

## GWNUM C API Reference

### Key Types

```c
typedef struct gwhandle { ... } gwhandle;  // Opaque context (thread-local)
typedef double *gwnum;                      // FFT-domain number
```

### Initialization

```c
// Initialize gwhandle structure (must call before gwsetup)
void gwinit2(gwhandle *gwdata, int struct_size, const char *version_string);

// Configure for modular arithmetic mod k*b^n+c
// Returns 0 on success
int gwsetup(gwhandle *gwdata, double k, unsigned long b, unsigned long n, signed long c);

// Cleanup (frees all gwnums and internal state)
void gwdone(gwhandle *gwdata);
```

### Memory Management

```c
gwnum gwalloc(gwhandle *gwdata);           // Allocate one gwnum
void  gwfree(gwhandle *gwdata, gwnum g);   // Free one gwnum
void  gwfreeall(gwhandle *gwdata);         // Free all gwnums in this context
```

### Arithmetic (IBDWT - modular reduction is free inside FFT)

```c
// Multiply: d = s1 * s2 mod N
void gwmul3(gwhandle *gwdata, gwnum s1, gwnum s2, gwnum d, int options);

// Square: d = s^2 mod N (macro expanding to gwmul3)
#define gwsquare2(h, s, d, opt) gwmul3(h, s, s, d, opt)

// Add/subtract
void gwadd3o(gwhandle *gwdata, gwnum s1, gwnum s2, gwnum d, int options);
void gwsub3o(gwhandle *gwdata, gwnum s1, gwnum s2, gwnum d, int options);
```

### Conversion (rug::Integer <-> gwnum)

```c
// Binary array to gwnum
void binarytogw(gwhandle *gwdata, unsigned int *array, int len, gwnum g);

// gwnum to binary array
void gwtobinary(gwhandle *gwdata, gwnum g, unsigned int *array, int len);
```

### Error Detection

```c
// Check for roundoff errors after arithmetic operations
#define gw_test_for_error(h) ((h)->GWERROR)
```

### Thread Safety Model

- Each thread MUST have its own `gwhandle`
- gwnums allocated from one handle CANNOT be used with another
- No global state; all state is in the `gwhandle` struct
- This maps naturally to Rust's `!Send + !Sync` pattern

---

## PFGW Reference

### Binary

PFGW (Primes or Fermats, George Woltman) is a general-purpose PRP/proof tool. Available as `pfgw64` binary for x86-64 Linux/macOS/Windows.

### Invocation Patterns

```bash
# PRP test (default)
pfgw64 input.txt

# N-1 proof (Pocklington — for N where N-1 has known factors)
pfgw64 -tp input.txt

# N+1 proof (Morrison — for N where N+1 has known factors)
pfgw64 -tm input.txt
```

### Input File Format

One expression per line:

```
# Factorial
123456!+1
123456!-1

# Primorial
1021#+1
1021#-1

# Wagstaff
(2^42737+1)/3

# Algebraic expression (near-repdigit)
10^1001 - 1 - 4*(10^600 + 10^400)

# Repunit
(10^1000-1)/9

# Decimal number (palindromic)
100000000000000000000000000000000000000001
```

### Output Parsing

```
"is prime!"              -> Prime (deterministic proof)
"is a probable prime"    -> PRP (probabilistic)
"PRP"                    -> PRP (probabilistic)
"is not prime"           -> Composite
"composite"              -> Composite
```

When using `-tp` or `-tm`, successful proof output includes "is prime!" with proof details.

---

## FLINT Integration

### Rationale

FLINT's `fft_small` module provides SIMD-vectorized NTTs for fast large-integer multiplication:
- **x86-64**: Uses AVX2 instructions
- **Apple Silicon**: Uses NEON instructions
- 3-10x faster than GMP for numbers above ~10,000 digits
- BSD license (no GPL concerns)
- Rust bindings: `flint3-sys` 3.3.1 on crates.io

### Key Functions

```c
// Factorial computation (binary splitting, faster than GMP for very large n)
void fmpz_fac_ui(fmpz_t f, ulong n);

// Primorial computation
void fmpz_primorial(fmpz_t res, ulong n);

// General multiplication (uses fft_small internally for large operands)
void fmpz_mul(fmpz_t f, const fmpz_t g, const fmpz_t h);
```

### rug <-> FLINT Conversion

Both GMP and FLINT use `mpz_t` internally. Conversion is zero-copy when sharing the underlying limb data:

```rust
// rug::Integer uses GMP's mpz_t internally
// FLINT's fmpz uses either small (inline) or mpz_t representation
// For large numbers, both store as mpz_t → pointer swap possible
```

### Build Requirements

```bash
# Linux
apt install libflint-dev

# macOS
brew install flint

# Cargo feature
cargo build --features flint
```

---

## Vrba-Reix Test for Wagstaff Primes

The only known efficient test for Wagstaff numbers (2^p+1)/3.

### Algorithm

```
Input: prime p >= 3
N = (2^p + 1) / 3

1. S(0) = 3/2 mod N
   (compute as: 3 * modular_inverse(2, N) mod N)
2. For i = 1 to p-2:
     S(i) = S(i-1)^2 - 2 mod N
3. N is (probable) prime iff S(p-2) == 0 mod N
```

### GWNUM Implementation

Using GWNUM's IBDWT for the squaring loop:
- Setup: `gwsetup(gwdata, 1.0/3.0, 2, p, 1)` configures for mod (2^p+1)/3
- Each iteration: one `gwsquare2` + one `gwsub` (for -2)
- Total: p-2 squarings (same complexity as Lucas-Lehmer)
- With Gerbicz checking every sqrt(p) steps

### Performance at Frontier

| p | Digits | GMP squarings/sec | GWNUM squarings/sec | Total time |
|---|--------|-------------------|--------------------|-----------|
| 100K | 30K | ~50/s | ~5000/s | ~20 sec |
| 1M | 300K | ~0.5/s | ~50/s | ~5.5 hours |
| 5M | 1.5M | ~0.02/s | ~2/s | ~29 days |
| 10M | 3M | ~0.005/s | ~0.5/s | ~231 days |

---

## Gerbicz Error Checking

### Overview

Gerbicz-Li error checking detects hardware errors (bit flips, CPU bugs) during long computations. Essential for multi-day tests where a single error invalidates the entire result.

### Algorithm (for squaring sequences)

```
Given: squaring sequence u_{i+1} = u_i^2 mod N
Block size: L = floor(sqrt(total_iterations))

Every L iterations, verify:
  d = u_{checkpoint} (saved value)
  For L steps: d = d * u_{i} for each intermediate
  Check: d^2 == u_{current} (mod N)
```

### Integration Points

- PRST v13.3+ includes Gerbicz checking natively
- GWNUM `gwnum_llr()`: checkpoint every sqrt(n) iterations
- GMP fallback in `kbn.rs`: add Gerbicz verification to existing LLR checkpoint infra

---

## Build Instructions

### PFGW (Phase 0)

1. Download `pfgw64` from [PrimeForm download page](https://sourceforge.net/projects/openpfgw/)
2. Place in PATH or specify via `--pfgw-path`
3. Verify: `pfgw64 --version`

### FLINT (Phase 1)

```bash
# Install system library
brew install flint    # macOS
apt install libflint-dev  # Linux

# Build with feature
cargo build --features flint
cargo test --features flint
```

### GWNUM (Phase 3)

```bash
# gwnum-sys crate handles build automatically via build.rs
# Requires: x86-64 platform, C compiler

# Option A: System-installed gwnum.a
# Place gwnum.a in /usr/local/lib/ and headers in /usr/local/include/

# Option B: Automatic build from Prime95 source
# build.rs downloads and compiles gwnum from mersenne.org

# Build with feature
cargo build --features gwnum
cargo test --features gwnum
```

---

## Deployment Checklist

### PFGW Deployment

- [ ] `pfgw64` binary available on all worker nodes
- [ ] Binary in PATH or `--pfgw-path` configured
- [ ] Verify: `pfgw64 --version` returns valid output
- [ ] Temp directory writable for input files
- [ ] Timeout configured (default: 1 hour per candidate)

### FLINT Deployment

- [ ] `libflint` installed on build machine
- [ ] Feature flag enabled: `cargo build --features flint`
- [ ] Benchmark: compare GMP vs FLINT at target factorial sizes

### GWNUM Deployment

- [ ] x86-64 platform confirmed (GWNUM uses x86 assembly)
- [ ] `gwnum.a` built or installed
- [ ] Feature flag enabled: `cargo build --features gwnum`
- [ ] Gerbicz error checking enabled for multi-day tests
- [ ] Cross-verification: test known primes with both GWNUM and GMP

---

## Performance Models

### Time per candidate by tool and size

```
Digits    GMP MR(25)    PFGW PRP    GWNUM squaring
1K        ~10ms         ~5ms        N/A (overhead)
10K       ~500ms        ~50ms       ~10ms
100K      ~5min         ~30sec      ~5sec
1M        ~50hr         ~30min      ~5min
3M        ~12mo         ~2wk        ~2days
```

### Crossover points

- **PFGW vs GMP**: ~5,000 digits (PFGW wins above this)
- **GWNUM vs GMP**: ~3,000 digits (GWNUM wins above this)
- **GWNUM vs PFGW**: Roughly equal for PRP; GWNUM wins for custom tests (Vrba-Reix, progress reporting)

---

## Risk Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| PFGW binary not available on ARM | High | Fall back to GMP; PFGW is x86-64 only |
| gwnum.a build fails on CI | High | Vendor pre-built .a in gwnum-sys; feature-gate CI |
| GWNUM roundoff errors at extreme sizes | Medium | Gerbicz checking detects errors; retry with larger FFT |
| FLINT 3.4.0 breaks flint3-sys 3.3.1 | Medium | Pin FLINT 3.3.x or patch flint3-sys |
| gwhandle thread-safety violations | High | PhantomData<!Send> + wrapper design prevents cross-thread use |

---

## Cross-Verification Matrix

After discovery, re-test with a different tool:

| Discovery tool | Verification tool | Command |
|---------------|-------------------|---------|
| GMP MR | PFGW PRP | `pfgw64 input.txt` |
| PFGW PRP | GMP MR(50) | `cargo run -- verify --id N` |
| PRST Proth | PFGW -tp | `pfgw64 -tp input.txt` |
| GWNUM Vrba-Reix | PFGW PRP | `pfgw64 input.txt` |
| GWNUM Proth | PRST | `prst input.txt` |

Rule: no discovery is announced until verified by an independent tool.
