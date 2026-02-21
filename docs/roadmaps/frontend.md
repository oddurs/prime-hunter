# Frontend Roadmap

Next.js dashboard: data exploration, visualization, search management, and documentation.

**Key files:** `frontend/`

---

## Current State

The webapp is a read-only dashboard: three stat cards, a paginated primes table, a search status indicator, and a docs viewer. Data flows one way — the CLI writes to SQLite, the dashboard reads it. The WebSocket pushes updates every 2 seconds.

---

## Phase 1: Data Exploration & Filtering

### Server-side filtering

Add query parameters to `/api/primes` and extend the WebSocket `get_primes` action:

```
GET /api/primes?form=factorial&min_digits=100&max_digits=1000&search=427
```

**Backend changes (db.rs):** Add `get_primes_filtered(filters: PrimeFilter)` with dynamic WHERE clause construction. Filter fields: `form`, `min_digits`/`max_digits`, `expression` (LIKE search), date range. Add index: `CREATE INDEX idx_primes_form_digits ON primes(form, digits)`.

**Frontend changes (page.tsx):** Collapsible filter bar with form dropdown, digit range inputs, expression search, date range picker. Filter state in URL search params for shareability. Debounce text search (300ms).

### Column sorting

Clickable column headers with sort direction indicators. Validate column name server-side to prevent SQL injection.

### Result export

- `GET /api/export?format=csv` / `GET /api/export?format=json`
- Streams with current filters applied, `Content-Disposition: attachment`
- Export dropdown button in table header

### Prime detail view

Click a row to show full details: expression, digit count, timestamp, search parameters, primality test method, link to relevant docs.

---

## Phase 2: Visualization

### Discovery timeline

Stacked area chart (recharts): x-axis = time, y-axis = cumulative count, colored by form. Time range selector (24h / 7d / 30d / all).

### Digit distribution histogram

Bar chart: x-axis = digit count ranges, y-axis = count, colored by form. Adjustable bucket size (10 / 100 / 1000). Log scale toggle.

### Search throughput gauge

Large numeric display of candidates/sec. Sparkline for last 5 minutes. ETA based on current rate and remaining range.

### Form-specific insights

- **Factorial:** Plot discovered n values against known primes, highlight gaps
- **Palindromic:** Digit count distribution, leading digit breakdown, density chart
- **KBN:** n-value distribution, rate vs estimated density comparison

---

## Phase 3: Search Management from the Web

### Subprocess model

Dashboard spawns search processes as children, monitors via stdout/stderr + checkpoint files. Each search gets its own checkpoint file.

**API:**
- `POST /api/searches` — start new search
- `GET /api/searches` — list all (running + completed)
- `DELETE /api/searches/{id}` — cancel (SIGTERM)

### Search configuration UI

New `/searches` page with:
- Search type selector (Factorial / Palindromic / KBN)
- Dynamic form fields based on type
- Active searches list with position, rate, cancel button
- Search history with re-run button

### Search presets & suggestions

Curated ranges from research docs:
- **Factorial:** "Quick verification" (1..100), "Explore uncharted" (1000..10000), "Deep search" (10000..50000)
- **Palindromic:** "Base 10 small" (1..9 digits), "Base 10 medium" (9..15), "Binary palindromes" (1..31)
- **KBN:** "Mersenne-like" (k=1,b=2), "Proth primes" (k=3,b=2), "Generalized Fermat" (k=1,b=10)

### Search queue

FIFO queue with configurable concurrency (default: 1). Persist to disk for dashboard restart survival.

---

## Phase 4: Enhanced Documentation

### Full-text search

Search across all doc content with highlighted matching snippets.

### Math rendering

Already using rehype-katex + remark-math. Continue converting doc expressions to LaTeX notation.

### Interactive cross-references

Link discovered primes to documentation. Form badge links to docs; docs sidebar shows prime counts.

### Known primes comparison

Cross-reference discoveries with OEIS sequences. Annotate: "This is known" vs "Not in known lists — potential new discovery!"

---

## Phase 5: Observability & Performance

### Progress file protocol

Search processes write JSON every 10s: `{ "tested": N, "found": M, "rate_per_sec": X, "uptime_secs": N }`. Dashboard reads alongside checkpoint.

### Performance dashboard page

New `/performance` page with:
- Throughput panel (rate, history chart, per-core efficiency)
- Sieve effectiveness ("Sieve filtered 94.3% of candidates")
- Resource utilization (CPU, memory, DB size, checkpoint age)

### ETA estimation

Progress bar with percentage and time estimate. Account for non-linear scaling (factorial tests slow down as n grows).

---

## Phase 6: Result Verification & Sharing

### Verification workflow

`POST /api/primes/{id}/verify` triggers re-test with higher MR rounds. Store verification status in database.

### Shareable result pages

`/prime/{id}` permalink with expression, digit count, discovery details, verification status.

### OEIS cross-reference

Automate checking discoveries against known OEIS sequences (A002981, A002982, A002385).

---

## Phase 7: Multi-Node Coordination

> **Architecture note (Feb 2026):** Network coordination now uses PG-only model. Operator pages (`/network`) show node status. Role-based navigation distinguishes operator vs admin views. The fleet page has been renamed to network.

### Agent architecture

Each server runs `darkreach agent --server <url> --token <token>`. Agents connect via WebSocket, receive assignments, report progress.

### Work distribution

- **Factorial:** Split n-range across nodes
- **KBN:** Split n-range (clean, no dependencies)
- **Palindromic:** Split by digit count or half-value range

### Node monitoring page

New `/network` page: hostname, cores, assignment, throughput, heartbeat, uptime. Aggregate throughput across network.

### Authentication

Token-based auth with `--admin-token` flag. API key validation middleware in Axum.

---

## Phase 8: Algorithm Improvements in UI

### Algorithm selection

Auto-suggest optimal algorithm when user enters kbn parameters. Show speed vs generality tradeoff.

### Sieve configuration

Sieve depth slider with estimated memory usage and effectiveness improvement.

### Adaptive Miller-Rabin rounds

Display false positive probability. Recommendation engine for round count.

---

## Phase 9: Polish & Production

- Responsive design (mobile support)
- Dark/light mode toggle (next-themes)
- Browser notifications (prime found, search completed/failed)
- PWA support (manifest.json, service worker)
- WebSocket reconnection with exponential backoff
- Docker multi-stage build (Rust + Next.js)

---

## Implementation Priority

| Phase | Effort | Impact | Dependencies |
|-------|--------|--------|-------------|
| 1. Data exploration | Medium | High | None |
| 2. Visualization | Medium | High | Phase 1 |
| 3. Search management | Large | Very High | None |
| 4. Enhanced docs | Small | Medium | None |
| 5. Observability | Medium | High | Phase 3 |
| 6. Verification | Medium | Medium | Phase 1 |
| 7. Multi-node | Very Large | High | Phase 3 |
| 8. Algorithm UI | Large | Medium | Phase 3 |
| 9. Polish | Medium | Medium | All |

**Recommended order:** 1 -> 4 -> 2 -> 3 -> 5 -> 6 -> 9 -> 7 -> 8
