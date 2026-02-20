# Server Setup & Cost Analysis

Optimal infrastructure for running darkreach at scale. Analysis as of February 2026.

---

## TL;DR — Phased Deployment

**Don't buy hardware before the software is ready.** GWNUM integration provides 50-100x speedup for large candidates — worth more than $10,000/month in hardware. Start small, scale with software milestones.

| Phase | Gate | Server(s) | Cores | $/mo | Target |
|-------|------|-----------|-------|------|--------|
| **1. Now** | None | 1x AX42 | 8 | **$53** | Sierpinski/Riesel (works today, best ROI) |
| **2. GWNUM works** | 50x+ verified speedup | +1-2x AX162-S | 56-104 | **$215-430** | Wagstaff, palindromic record |
| **3. Full fleet** | +Gerbicz + coordination | +AX102 + AX162-S | 120-168 | **$550-760** | Factorial, parallel campaigns |

See [ops.md Phased Deployment](roadmaps/ops.md#phased-deployment-plan) for the full rationale.

---

## Why These Choices

### GMP Performance Is Everything

darkreach spends >99% of CPU time in GMP's `mpz_powm` (modular exponentiation) and `mpz_mul` (multiplication). The critical CPU features:

- **MULX instruction (BMI2)**: Allows multiply without clobbering flags. GMP's inner loops depend on this.
- **ADCX/ADOX (ADX extension)**: Two independent carry chains in parallel. This is what makes x86 faster than ARM for GMP.
- **High clock speed**: `is_probably_prime()` is single-threaded per candidate. Higher clocks = faster individual tests.
- **Many cores**: darkreach parallelizes across candidates via rayon. Total throughput = single-core speed x core count.

### AMD Zen 4 Is the Sweet Spot

| Architecture | GMPbench (single-thread) | Notes |
|-------------|------------------------|-------|
| AMD Zen 4 (Ryzen 7950X) | ~9,480 | Highest score. Best for darkreach. |
| Intel Alder Lake (i5-12600K) | ~7,950-9,234 | Competitive but fewer cores per dollar. |
| AMD Zen 3 (Ryzen 5950X) | ~6,864-8,094 | Still good. Cloud instances use this. |
| Apple M2 (ARM) | ~6,445 | Good per-watt, but no server availability. |
| AMD EPYC 9454P (Zen 4, server) | ~7,500-8,000 est. | Lower clocks than desktop but 48 cores. |

### WARNING: Avoid AMD Zen 5

GMP has issued an [official warning](https://gmplib.org/gmp-zen5) about AMD Zen 5 (Ryzen 9000, EPYC 9005 "Turin"). Two Ryzen 9950X CPUs were physically destroyed running sustained GMP workloads. The tight assembly loops drawing one MULX per cycle appear to exceed Zen 5's power envelope. **Do not run darkreach on Zen 5 hardware until AMD resolves this.**

---

## Provider Comparison

### Hetzner Dedicated (Best Value)

| Model | CPU | Cores | RAM | EUR/mo | ~USD/mo | $/core |
|-------|-----|-------|-----|--------|---------|--------|
| AX42 | Ryzen 7 PRO 8700GE | 8 | 64 GB DDR5 | 49 | ~53 | $6.63 |
| AX102 | Ryzen 9 7950X3D | 16 | 128 GB DDR5 | 110 | ~119 | $7.44 |
| **AX162-S** | **EPYC 9454P** | **48** | **128 GB DDR5** | **199** | **~215** | **$4.48** |
| AX162-R | EPYC 9454P | 48 | 256 GB DDR5 | 199 | ~215 | $4.48 |

The AX162-S is the clear winner for sustained compute. 48 Zen 4 cores at $4.48/core/month is unmatched. The AX102's Ryzen 7950X3D has the highest single-core clocks (up to 5.7 GHz boost) plus 3D V-Cache, making it ideal for the sequential factorial computation.

### Hetzner Cloud CCX (Flexible)

| Plan | vCPUs | RAM | ~USD/mo | $/vCPU |
|------|-------|-----|---------|--------|
| CCX33 | 8 | 32 GB | ~52 | $6.56 |
| CCX43 | 16 | 64 GB | ~104 | $6.50 |
| CCX53 | 32 | 128 GB | ~208 | $6.50 |

Hourly billing. Good for short campaigns — spin up 10 instances for a week, tear down when done. ~45% more expensive per core than dedicated servers for 24/7 use.

### OVH Dedicated

| Model | CPU | Cores | USD/mo | $/core |
|-------|-----|-------|--------|--------|
| Advance-5 2024 | EPYC 8224P | 24 | $236 | $9.83 |
| Scale-a3 | EPYC 9354 | 32 | $513 | $16.03 |

2-3x Hetzner's $/core. Only consider if Hetzner is sold out.

### AWS EC2 (Burst Campaigns Only)

| Instance | vCPUs | On-Demand $/mo | Spot $/mo | $/vCPU (spot) |
|----------|-------|----------------|-----------|---------------|
| c7a.4xlarge | 16 | ~$591 | ~$257 | $16.06 |
| c7a.8xlarge | 32 | ~$1,182 | ~$492 | $15.38 |
| c7a.16xlarge | 64 | ~$2,365 | ~$954 | $14.91 |
| c7i.16xlarge | 64 | ~$2,056 | ~$643 | $10.05 |

3-5x more expensive than Hetzner dedicated, even with spot pricing. Spot instances can be interrupted — fine for kbn/palindromic (checkpointing saves progress) but bad for factorial (sequential computation). Use AWS only for short high-parallelism bursts, e.g., sieving a massive kbn range across 100+ instances in a weekend.

### Others (Not Recommended)

| Provider | $/core/mo | Verdict |
|----------|-----------|---------|
| Vultr bare metal | $30+ | Overpriced for compute. |
| DigitalOcean | $21+ | Not competitive. |

---

## Fleet Architecture

```
                    ┌─────────────────────┐
                    │   AX102 (16 cores)  │
                    │   Dashboard + Docs  │
                    │   Factorial search   │
                    │   Port 7001 public   │
                    └─────────┬───────────┘
                              │ WebSocket / REST
              ┌───────────────┼───────────────┐
              │               │               │
    ┌─────────▼─────────┐  ┌─▼─────────────┐ │
    │  AX162-S #1        │  │  AX162-S #2   │ ...
    │  48 cores          │  │  48 cores     │
    │  kbn search        │  │  palindromic  │
    │  --coordinator     │  │  --coordinator│
    │    http://AX102    │  │    http://AX102│
    └────────────────────┘  └───────────────┘
```

Each worker registers with the coordinator dashboard, sends heartbeats every 10s, and reports discovered primes. The dashboard aggregates stats, shows fleet status, and stores all results in SQLite.

### Deployment

```bash
# From your machine, deploy to each server:
./deploy/deploy.sh root@ax162-1.example.com --coordinator http://ax102.example.com:7001
./deploy/deploy.sh root@ax162-2.example.com --coordinator http://ax102.example.com:7001
./deploy/deploy.sh root@ax102.example.com   # coordinator (no --coordinator flag)
```

Systemd manages all services with `Restart=always`. See `deploy/darkreach-coordinator.service` and `deploy/darkreach-worker@.service`.

---

## Cost Scenarios

### Phase 1: Foundation ($53/mo) — START HERE

1x Hetzner AX42 (8 Zen 4 cores, 64 GB RAM).

- **Run non-base-2 Sierpinski/Riesel searches immediately** (best ROI, works with current code)
- Ship all software optimizations: BSGS sieving, Proth/LLR, sieve improvements
- Develop and test GWNUM integration (or PRST/PFGW subprocess)
- Dashboard co-hosted
- Can search kbn ranges up to n~500K comfortably
- **Realistic chance of discovering primes that solve conjectures within months**

### Phase 2: After GWNUM Works ($215-430/mo)

AX42 + 1-2x AX162-S = 56-104 cores.

- **Gate: GWNUM integration verified with 50x+ speedup at 500K+ digits**
- AX162-S servers run Wagstaff gap search (p=4M-15M) and palindromic record attempts
- AX42 continues Sierpinski/Riesel + dashboard
- Candidates at 1.5M+ digits now feasible (days instead of months)
- Start palindromic record campaign at 3M+ digits

### Phase 3: Full Fleet ($550-760/mo)

Add AX102 + 1-2 more AX162-S = 120-168 cores.

- **Gate: GWNUM + Gerbicz error checking + distributed coordination all working**
- AX102 runs factorial search (best single-core at 5.7 GHz) + dashboard
- AX162-S servers run parallel campaigns across search types
- Split non-base-2 Sierpinski/Riesel conjectures across workers
- Multi-month sustained search campaigns for records

### Annual Cost Comparison

| Phase | Monthly | Annual | AWS Equivalent (on-demand) |
|-------|---------|--------|---------------------------|
| Phase 1 (foundation) | $53 | $636 | ~$500/mo = $6,000/yr |
| Phase 2 (after GWNUM) | $215-430 | $2,580-5,160 | ~$2,000-4,000/mo = $24K-48K/yr |
| Phase 3 (full fleet) | $550-760 | $6,600-9,120 | ~$5,000-7,500/mo = $60K-90K/yr |

---

## What Each Search Type Needs

### Factorial (n! +/- 1)

- **Bottleneck**: Sequential n! computation (single-core), then parallel primality test
- **Best CPU**: Highest single-core clocks (Ryzen 7950X3D)
- **RAM**: ~2-4 GB per candidate at n~500K. 64 GB is plenty.
- **Cores used**: 1 core for factorial computation, 2 cores for parallel +1/-1 testing
- **Disk**: Negligible (checkpoint files only)

### kbn (k*b^n +/- 1)

- **Bottleneck**: Primality testing (Proth/LLR for base-2, Miller-Rabin for others)
- **Best CPU**: Many cores. Each candidate tests independently.
- **RAM**: ~1-2 GB per candidate at n~100K. 128 GB supports 48 parallel tests.
- **Cores used**: All of them. Fully parallelizable via rayon.
- **Disk**: Negligible

### Palindromic

- **Bottleneck**: Primality testing of candidates that survive sieve
- **Best CPU**: Many cores. Batch generation + parallel testing.
- **RAM**: ~1-4 GB depending on digit count
- **Cores used**: All of them. Fully parallelizable.
- **Disk**: Negligible

---

## Network Requirements

- **Bandwidth**: Minimal. Worker heartbeats are ~200 bytes every 10s. Prime reports are ~500 bytes each. Total fleet traffic is under 1 Mbps even with 100 workers.
- **Latency**: Not critical. Heartbeat tolerance is 60s before a worker is pruned.
- **Firewall**: Only the coordinator needs port 7001 open. Workers make outbound HTTP connections.

All Hetzner servers include 1 Gbps unmetered (AX series) or 20 TB/mo (cloud), which is vastly more than needed.

---

## References

- [Hetzner AX Dedicated Servers](https://www.hetzner.com/dedicated-rootserver/matrix-ax/)
- [Hetzner Cloud Pricing](https://www.hetzner.com/cloud/)
- [GMPbench Results](https://gmplib.org/gmpbench)
- [GMP Zen 5 Warning](https://gmplib.org/gmp-zen5)
- [AWS EC2 Pricing (Vantage)](https://instances.vantage.sh/)
