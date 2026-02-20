# Ops Roadmap

Deployment, build optimization, fleet management, and hardware-specific tuning.

**Key files:** `deploy/`

---

## Apple Silicon Optimization

The development machine runs macOS/Darwin (aarch64). These optimizations are critical.

### GMP on Apple Silicon

Apple M1 ranks #6 on the GMPbench leaderboard at 3.2 GHz. Key primitive performance:
- `mpn_mul_1`: 1.0 cycles/limb (matches best x86)
- `mpn_addmul_1`: 1.25 c/l (bottlenecked by ARM's single carry flag vs x86's dual adcx/adox)

GMP's aarch64 assembly is less mature than x86-64 but Apple's microarchitecture compensates.

### Build Configuration

```toml
# .cargo/config.toml
[target.aarch64-apple-darwin]
rustflags = ["-C", "target-cpu=apple-m1"]
```

**CRITICAL: Do NOT use `-Ctarget-cpu=native`** — Rust bug #93889 resolves it to `cyclone` (2013 chip), producing WORSE code than the default.

### Threading Strategy

```rust
// Pin prime hunting to P-cores with elevated QoS
use rayon::ThreadPoolBuilder;

ThreadPoolBuilder::new()
    .num_threads(num_p_cores)  // P-cores only, not E-cores
    .spawn_handler(|thread| {
        std::thread::Builder::new().spawn(move || {
            unsafe {
                libc::pthread_set_qos_class_self_np(
                    libc::QOS_CLASS_USER_INITIATED, 0,
                );
            }
            thread.run();
        })?;
        Ok(())
    })
    .build_global()
    .unwrap();
```

Benchmark BOTH configurations (P-cores only vs all cores) — Mlucas found all 8 cores faster on M1 for Mersenne testing, but this varies by workload.

### Quick Wins

| Action | Impact |
|--------|--------|
| Use `apple-m1` target CPU (not `native`) | Correct codegen |
| Set Rayon thread QoS to `user-initiated` | P-core scheduling |
| Add `mimalloc` as global allocator | ~10x faster large allocations |
| Profile-Guided Optimization (PGO) | 8-12% overall improvement |
| Use Mac Mini/Studio for sustained work (not Air) | Avoid thermal throttling |
| Ensure GMP built with assembly (`brew install gmp`) | Maximum primitive speed |

### Not Worth Pursuing on Apple Silicon

- **AMX coprocessor**: Wrong operand widths (16-bit max multiply), undocumented
- **vDSP/Accelerate FFT**: Floating-point only, not exact arithmetic
- **Metal GPU sieving**: 64-bit integer multiply is 8 cycles (terrible), 8KB L1 cache
- **Prime95**: Not optimized for ARM, would need Rosetta 2 translation

### FLINT on Apple Silicon

FLINT 3's small-prime FFT supports ARM NEON (`--enable-neon`). For numbers above ~10K digits, FLINT is 3-10x faster than GMP (single-core to 8-core). This is the most promising library upgrade path for Apple Silicon.

---

## GPU-Accelerated Sieving

**Current:** CPU-only sieving.

**Target:** Offload sieving to GPU via OpenCL or CUDA. The BSGS sieving algorithm is embarrassingly parallel and maps well to GPU architectures.

**Precedent:** mtsieve's `srsieve2cl` (OpenCL), `mfaktc` (CUDA for Mersenne trial factoring), `CUDA-Riesel-Sieve`.

**Rationale:** GPU parallelism can push sieve depth to 10^12+ where CPU sieving becomes impractical.

---

## Build Configuration Best Practices

Current release profile in `Cargo.toml`:
```toml
[profile.release]
lto = "fat"           # Link-time optimization (whole program)
codegen-units = 1     # Single codegen unit for better optimization
opt-level = 3         # Maximum optimization level
```

Additional optimization opportunities:
- Profile-Guided Optimization (PGO): collect profile from real workload, rebuild
- `mimalloc` global allocator for faster large allocations
- Ensure GMP is built with architecture-specific assembly

---

## Observability Defaults

The coordinator persists system logs and time-series metrics for long-term dashboards.

- **Metric sampling:** every 60s for coordinator/fleet, every 120s for per-worker samples
- **Raw retention:** 7 days (metrics)
- **Hourly rollups:** 365 days
- **Daily rollups:** 1825 days
- **Log retention:** 30 days

Environment overrides (optional):
- `OBS_LOG_RETENTION_DAYS`
- `OBS_METRIC_RETENTION_DAYS`
- `OBS_ROLLUP_RETENTION_DAYS`
- `OBS_DAILY_ROLLUP_RETENTION_DAYS`
- `OBS_ERROR_BUDGET_ERRORS_PER_HOUR`
- `OBS_ERROR_BUDGET_WARNINGS_PER_HOUR`
- `NEXT_PUBLIC_ERROR_BUDGET_ERRORS_PER_HOUR`
- `NEXT_PUBLIC_ERROR_BUDGET_WARNINGS_PER_HOUR`

API endpoints:
- `GET /api/observability/metrics`
- `GET /api/observability/logs`
- `GET /api/observability/report`

---

## Fleet Deployment

### Current Architecture

- `deploy/deploy.sh` — SSH deployment script: installs Rust/GMP, clones/updates repo, builds with `-C target-cpu=native`, copies binary to `/usr/local/bin`, installs systemd units
- `deploy/darkreach-coordinator.service` — Systemd unit for dashboard on port 7001, security-hardened (strict filesystem, no new privs, 2GB memory limit)
- `deploy/darkreach-worker@.service` — Template unit for workers with `--coordinator` flag, supports instance numbers, auto-restarts every 10s

### Deployment Flow

```
deploy.sh → SSH to target → install deps → clone/pull → cargo build --release → systemctl enable/start
```

Workers register with coordinator via HTTP heartbeat every 10 seconds. Stale workers pruned after 60s timeout.

### Automation Improvements

- Ansible or similar for multi-host deployment
- Health monitoring and alerting
- Automated binary distribution (avoid building on each host)
- Log aggregation from systemd journal

---

## Phased Deployment Plan

**Core principle: Scale software before hardware.** GWNUM integration provides 50-100x speedup at large number sizes — worth more than $10,000/month in hardware. Buying a fleet before GWNUM works is burning money.

Full server cost analysis and provider comparison: [server-setup.md](../server-setup.md)

### Phase 1: Foundation ($53/mo)

**Hardware:** 1x Hetzner AX42 (Ryzen 7 PRO 8700GE, 8 cores, 64 GB DDR5)

**Gate:** Nothing — start now.

**What runs:**
- Non-base-2 Sierpinski/Riesel searches (best ROI: 1-10 core-years per discovery)
- All software optimization work (BSGS, Proth, LLR, sieve improvements)
- GWNUM integration development and testing
- Dashboard co-hosted on same machine

**Why this is enough:** Sierpinski/Riesel at n<500K uses GMP efficiently. 8 Zen 4 cores can test ~100-500 candidates/day. With 1-10 core-years per discovery, a single machine has realistic odds within months.

### Phase 2: After GWNUM ($215-430/mo)

**Hardware:** Add 1-2x Hetzner AX162-S (EPYC 9454P, 48 cores each)

**Gate:** GWNUM integration working (or PRST/PFGW subprocess reliable). Verified 50x+ speedup on candidates above 500K digits.

**What unlocks:**
- Wagstaff PRP: testing at p~5M (1.5M digits) drops from weeks to ~3 days/candidate with GWNUM
- Palindromic record: near-repdigit search at 3M+ digits becomes feasible
- Sierpinski/Riesel at higher n-values (millions of digits)

**Why wait:** Without GWNUM, these 48-core servers would run GMP at 1/50th to 1/100th the potential speed. Each core-month at GMP speed equals ~1 core-day at GWNUM speed for large candidates.

### Phase 3: Full Fleet ($550-760/mo)

**Hardware:** Add AX102 (Ryzen 7950X3D, 16 cores) for factorial + 1-3x more AX162-S

**Gate:** GWNUM + Gerbicz error checking both working. Distributed coordination implemented.

**What unlocks:**
- Factorial primes (sequential computation needs high single-core clocks, AX102's 5.7 GHz)
- Parallel campaigns across 100-160+ cores
- Multi-month sustained searches for records

**Why wait:** Factorial search at n~700K takes ~12-14 months per core with GMP. With GWNUM, this drops to weeks. Gerbicz checking is essential for tests lasting days — a hardware error at hour 47 of a 48-hour test wastes everything.

### The Math

| Target | With GMP (current) | With GWNUM | Speedup |
|--------|-------------------|------------|---------|
| Sierpinski/Riesel n=100K | Minutes | Minutes | ~1x (GMP fine here) |
| Wagstaff p=5M (1.5M digits) | Weeks | ~3 days | ~50x |
| Palindromic 3M digits | Infeasible | ~1 week | ~100x |
| Factorial n=700K (3.4M digits) | ~14 months/core | ~2 weeks/core | ~100x+ |

### Hardware Cost-Benefit Analysis

| Search Type | Core-years per discovery | Provable? | Competition | Deployment Phase |
|------------|------------------------|-----------|-------------|-----------------|
| Sierpinski/Riesel (non-base-2) | **1-10** | Yes (LLR/Proth) | Low | Phase 1 (now) |
| Palindromic record | **100-1,000** | Yes (BLS) | 1 team | Phase 2 |
| Wagstaff PRP | **~3,000** | No (PRP only) | None | Phase 2 |
| Factorial | **~2,300** | Yes (Pocklington/Morrison) | PrimeGrid | Phase 3 |
| Primorial | **~140,000** | Yes | PrimeGrid (not viable solo) | Not recommended |

Apple Silicon for development/testing. Hetzner Zen 4 dedicated servers for production search (better GMP assembly, no thermal throttling). **Avoid Zen 5** — see [server-setup.md](../server-setup.md#warning-avoid-amd-zen-5).
