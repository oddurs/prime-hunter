# frontend/ — Dashboard Domain

Next.js web dashboard for monitoring and managing the darkreach prime-hunting platform.

## Stack

| Technology | Version | Purpose |
|-----------|---------|---------|
| Next.js | 16 | React framework (static export mode) |
| React | 19 | UI library |
| Tailwind CSS | 4 | Utility-first styling |
| shadcn/ui | latest | Copy-paste component system (Radix UI) |
| Recharts | 3 | Charts (timeline, distribution, throughput) |
| Lucide React | latest | Icon library |
| Sonner | 2 | Toast notifications |
| @supabase/supabase-js | ^2 | Auth, queries, realtime |
| react-markdown + remark-math + rehype-katex | latest | LaTeX math in docs |

## Build & Dev

```bash
cd frontend
npm install
npm run dev        # Dev server with hot reload
npm run build      # Static export to frontend/out/
npm test           # Vitest unit tests
npm run test:e2e   # Playwright E2E tests
```

The static export is served by the Rust backend (`dashboard/mod.rs` serves `--static-dir`). No Node.js runtime needed.

## Data Flow

```
Frontend (Next.js static export)
├── Supabase Auth → login, session management
├── Supabase JS client → prime data (queries, stats, charts)
│   ├── use-primes.ts      → supabase.from("primes")
│   ├── use-stats.ts       → supabase.rpc("get_stats")
│   ├── use-timeline.ts    → supabase.rpc("get_discovery_timeline")
│   ├── use-distribution.ts → supabase.rpc("get_digit_distribution")
│   ├── use-records.ts     → supabase.from("world_records")
│   └── use-form-leaderboard.ts → supabase.rpc("form_leaderboard")
├── Supabase Realtime → live prime notifications
│   └── use-prime-realtime.ts → postgres_changes INSERT on primes
└── WebSocket → Rust backend (coordination only)
    └── use-websocket.ts → fleet, searches, deployments, status
```

**Rule:** Prime data comes from Supabase directly. WebSocket is only for coordination data.

## Directory Structure

```
src/
├── app/                           # Next.js pages (14 routes)
│   ├── layout.tsx                 # Root: AuthProvider, WebSocketProvider, ThemeProvider
│   ├── page.tsx                   # Main dashboard (stats, primes table, charts)
│   ├── login/page.tsx             # Email/password login (Supabase Auth)
│   ├── browse/page.tsx            # Primes browser (filter, paginate, detail dialog)
│   ├── searches/page.tsx          # Search job management
│   ├── network/page.tsx           # Network monitoring (nodes, health)
│   ├── agents/page.tsx            # AI agent management (tasks, budgets, memory)
│   ├── projects/page.tsx          # Project campaigns
│   ├── leaderboard/page.tsx       # Form leaderboard rankings
│   ├── performance/page.tsx       # Performance metrics and charts
│   ├── logs/page.tsx              # System logs viewer
│   ├── releases/page.tsx          # Worker release management
│   ├── prime/page.tsx             # Single prime detail (permalink)
│   └── docs/page.tsx              # Documentation viewer (markdown + KaTeX)
│
├── components/
│   ├── ui/                        # shadcn/ui primitives (17 components)
│   │   ├── badge, breadcrumb, button, card, dialog, dropdown-menu
│   │   ├── input, scroll-area, select, separator, sheet, skeleton
│   │   └── switch, table, tabs, textarea, tooltip
│   │
│   ├── charts/                    # Recharts visualizations
│   │   ├── discovery-timeline.tsx # Prime discoveries over time
│   │   ├── digit-distribution.tsx # Digit count histogram
│   │   ├── throughput-gauge.tsx   # Tests/sec gauge
│   │   └── cost-history.tsx       # Cost tracking chart
│   │
│   ├── agents/                    # Agent management components
│   │   ├── activity-feed.tsx      # Agent activity stream
│   │   ├── analytics-tab.tsx      # Agent analytics
│   │   ├── budget-cards.tsx       # Budget summary cards
│   │   ├── helpers.tsx            # Shared agent helpers
│   │   ├── memory-tab.tsx         # Agent memory viewer
│   │   ├── new-task-dialog.tsx    # Create agent task dialog
│   │   ├── schedules-tab.tsx      # Agent schedules
│   │   └── task-card.tsx          # Individual task card
│   │
│   ├── [Domain Components]
│   ├── app-header.tsx             # Navigation header (all page links)
│   ├── primes-table.tsx           # Paginated primes table
│   ├── prime-detail-dialog.tsx    # Full prime details modal
│   ├── prime-notifier.tsx         # Supabase Realtime toast notifications
│   ├── stat-card.tsx              # Dashboard stat card
│   ├── search-card.tsx            # Search configuration card
│   ├── search-job-card.tsx        # Search job status card
│   ├── new-search-dialog.tsx      # Create search dialog
│   ├── project-card.tsx           # Project campaign card
│   ├── new-project-dialog.tsx     # Create project dialog
│   ├── phase-timeline.tsx         # Project phase timeline
│   ├── host-node-card.tsx         # Host/node status card
│   ├── worker-detail-dialog.tsx   # Worker details modal
│   ├── service-status-card.tsx    # Service status indicator
│   ├── process-row.tsx            # Worker process row
│   ├── add-server-dialog.tsx      # Add server dialog
│   ├── agent-controller-card.tsx  # Agent controller card
│   ├── form-leaderboard.tsx       # Form ranking table
│   ├── record-comparison.tsx      # World record comparison
│   ├── cost-tracker.tsx           # Cost tracking display
│   ├── insight-cards.tsx          # Analytics insight cards
│   ├── metrics-bar.tsx            # Metrics status bar
│   ├── activity-feed.tsx          # Global activity feed
│   ├── view-header.tsx            # Page view header
│   ├── json-block.tsx             # JSON display block
│   ├── empty-state.tsx            # Empty state placeholder
│   └── darkreach-logo.tsx         # Brand logo component
│
├── contexts/
│   ├── auth-context.tsx           # Supabase Auth (AuthProvider, AuthGuard, useAuth)
│   └── websocket-context.tsx      # WebSocket (coordination data only)
│
├── hooks/                         # Custom hooks (18 total)
│   ├── [Supabase Data Hooks]
│   ├── use-primes.ts              # Prime records (PrimeRecord, PrimeFilter, PrimeDetail)
│   ├── use-stats.ts               # Dashboard stats via RPC
│   ├── use-timeline.ts            # Discovery timeline via RPC
│   ├── use-distribution.ts        # Digit distribution via RPC
│   ├── use-form-leaderboard.ts    # Form rankings via RPC
│   ├── use-records.ts             # World record data
│   ├── use-prime-realtime.ts      # Realtime INSERT subscription
│   │
│   ├── [WebSocket Hooks]
│   ├── use-websocket.ts           # Fleet, searches, deployments, status
│   │
│   ├── [Agent Hooks]
│   ├── use-agents.ts              # Agent list and status
│   ├── use-agent-tasks.ts         # Agent task management
│   ├── use-agent-budgets.ts       # Agent budget tracking
│   ├── use-agent-memory.ts        # Agent memory KV store
│   ├── use-agent-schedules.ts     # Agent schedule config
│   │
│   ├── [UI Hooks]
│   ├── use-theme.ts               # Dark/light toggle (localStorage: darkreach-theme)
│   ├── use-mobile.ts              # Mobile detection
│   ├── use-notifications.ts       # Browser notification permissions
│   ├── use-polling.ts             # Generic polling interval
│   └── use-projects.ts            # Project campaign data
│
├── lib/
│   ├── supabase.ts                # Supabase client singleton
│   ├── format.ts                  # Formatting utilities (numbers, dates, expressions)
│   └── utils.ts                   # Utility functions (cn class merger)
│
├── __tests__/                     # Vitest unit tests
│   ├── components/                # Component tests
│   ├── hooks/                     # Hook tests
│   ├── lib/                       # Utility tests
│   ├── pages/                     # Page tests
│   └── security/                  # Secret leak detection
│
└── __mocks__/                     # Test mocks
    ├── supabase.ts                # Supabase client mock
    └── test-wrappers.tsx          # Provider wrapper for tests
```

## Conventions

- **Components**: Use shadcn/ui from `components/ui/`. Add: `npx shadcn@latest add <name>`
- **Styling**: Tailwind utility classes only. No custom CSS except `globals.css`
- **State**: Supabase hooks for prime data; WebSocket for coordination. Local state for UI only
- **Types**: TypeScript strict mode. Types co-located in their respective hooks
- **Auth**: Supabase Auth (email/password). `AuthGuard` in layout gates all pages except `/login`
- **Icons**: Lucide React (`import { IconName } from "lucide-react"`)
- **Charts**: Recharts with responsive containers. Match dark theme colors
- **Notifications**: Sonner toasts for prime discoveries (via Supabase Realtime)
- **localStorage keys**: `darkreach-theme`, `darkreach-notifications-enabled`

## Environment Variables

```bash
NEXT_PUBLIC_SUPABASE_URL=https://xxx.supabase.co
NEXT_PUBLIC_SUPABASE_ANON_KEY=eyJ...
NEXT_PUBLIC_API_URL=https://api.darkreach.ai
NEXT_PUBLIC_WS_URL=wss://api.darkreach.ai/ws
```

## Agent Coding Guide

### Adding a new page

1. Create `src/app/<name>/page.tsx`
2. Add navigation link in `src/components/app-header.tsx`
3. Create data hook in `src/hooks/use-<name>.ts` (Supabase or WebSocket)
4. Use `AuthGuard` wrapper if auth required (most pages)
5. Use shadcn/ui components + Tailwind for styling

### Adding a new component

1. Create in `src/components/<name>.tsx` (or subdirectory for related group)
2. Export as default or named export
3. Accept typed props interface
4. Use shadcn/ui primitives (Card, Button, Badge, Table, Dialog, etc.)
5. Data fetching belongs in hooks, not components

### Adding a new hook

1. Create `src/hooks/use-<name>.ts`
2. For Supabase data: use `supabase.from()` or `supabase.rpc()`
3. For WebSocket data: consume from `WebSocketContext`
4. Return `{ data, loading, error, refetch? }` pattern
5. Add tests in `src/__tests__/hooks/`

### Adding a shadcn/ui component

```bash
cd frontend && npx shadcn@latest add <component-name>
```

Components are copied to `src/components/ui/` and are fully owned/customizable.

### Testing

- **Unit tests**: Vitest in `src/__tests__/`. Mock Supabase via `__mocks__/supabase.ts`
- **E2E tests**: Playwright in `e2e/`
- **Run**: `npm test` (unit), `npm run test:e2e` (E2E)

## Backend API Reference

The frontend calls these Rust backend endpoints (via WebSocket and REST):

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/ws` | WS | Real-time coordination (2s push) |
| `/api/health` | GET | Health check |
| `/api/status` | GET | Coordinator status |
| `/api/workers` | GET/POST | Worker list, heartbeat |
| `/api/fleet` | GET | Fleet overview |
| `/api/search_jobs` | GET/POST/PUT | Job CRUD |
| `/api/searches` | GET/POST | Search management |
| `/api/agents/*` | GET/POST/PUT | Agent tasks, budgets, memory |
| `/api/projects/*` | GET/POST/PUT | Project campaigns |
| `/api/verify` | POST | Prime re-verification |
| `/api/docs/*` | GET | Documentation content |
| `/api/observability/*` | GET | Metrics, logs, charts |
| `/api/releases/*` | GET/POST | Release channels |
| `/api/v1/operators/*` | GET/POST | Operator management |
| `/api/v1/nodes/*` | GET/POST | Node management |
| `/api/notifications/*` | GET/POST | Push notifications |

## Roadmap

See `docs/roadmaps/frontend.md` for planned features.
