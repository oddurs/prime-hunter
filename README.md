# darkreach

CLI tool for hunting special-form prime numbers using GMP for arbitrary-precision arithmetic and rayon for parallel search.

## Prime forms

- **Factorial primes**: n! ± 1
- **Palindromic primes**: primes that read the same forwards and backwards in a given base
- **k·b^n ± 1 primes**: covers Proth primes, Riesel primes, Mersenne-like forms, etc.

## Build

### Dependencies

**Linux (Ubuntu/Debian):**
```bash
apt install build-essential libgmp-dev m4
```

**macOS:**
```bash
brew install gmp
```

### Compile

```bash
cargo build --release
```

The binary will be at `target/release/darkreach`.

## Local Dev Stack

Recommended (single command, clean shutdown):

```bash
./scripts/dev.sh
```

This starts both services, waits for health checks, and stops both on Ctrl+C.

Use production dashboard data with local UI:

```bash
./scripts/dev.sh --remote https://your-prod-dashboard.example.com
```

This runs only local frontend on `:3001` and proxies `/api` + `/ws` to the remote dashboard.

Optional detached process helpers:

```bash
./scripts/dev-up.sh      # start backend (:7001) + frontend (:3001)
./scripts/dev-status.sh  # check process + HTTP health
./scripts/dev-down.sh    # stop both
```

Logs and PID files are stored in `.dev/`.

Standalone remote helper (equivalent to `dev.sh --remote`):

```bash
./scripts/dev-remote.sh https://your-prod-dashboard.example.com
```

This runs local Next.js on `:3000`, proxies `/api/*` to the remote host (avoids CORS), and connects WebSocket directly to remote `/ws`.

## Usage

### Factorial primes

Search for primes of the form n! ± 1:

```bash
darkreach factorial --start 1000 --end 50000
```

### Palindromic primes

Search for palindromic primes in a given base:

```bash
darkreach palindromic --base 10 --min-digits 50 --max-digits 200
```

Even-digit palindromes are automatically skipped (they're divisible by base+1).

### k·b^n ± 1 primes

Search for primes of the form k·b^n ± 1:

```bash
# Proth-style: 3·2^n + 1
darkreach kbn --k 3 --base 2 --min-n 100000 --max-n 500000

# Mersenne-like: 1·2^n - 1
darkreach kbn --k 1 --base 2 --min-n 1000 --max-n 100000
```

### Options

```
--db <path>          SQLite database path (default: darkreach.db)
--checkpoint <path>  Checkpoint file path (default: darkreach.checkpoint)
```

## Features

- **Resumable**: Checkpoints progress every 60 seconds. Restart the same command and it picks up where it left off.
- **Parallel**: Uses all available CPU cores via rayon. Core count printed at startup.
- **SQLite logging**: Every prime found is logged with form, expression, digit count, timestamp, and search parameters.
- **Progress reporting**: Status line every 30 seconds showing current candidate, test rate, and primes found.
- **Primality testing**: Uses GMP's Miller-Rabin with 25 iterations. Results marked as deterministic (small primes verified exactly) or probabilistic.

## Database schema

```sql
CREATE TABLE primes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    form TEXT NOT NULL,           -- "factorial", "palindromic", "kbn"
    expression TEXT NOT NULL,     -- e.g. "1000! + 1", "3*2^1234 - 1"
    digits INTEGER NOT NULL,     -- decimal digit count
    found_at TEXT NOT NULL,       -- ISO 8601 timestamp
    search_params TEXT NOT NULL   -- JSON of CLI args used
);
```

Query your results:

```bash
sqlite3 darkreach.db "SELECT expression, digits FROM primes ORDER BY digits DESC LIMIT 10;"
```
