# frontend/ — Dashboard Domain

Next.js web dashboard for monitoring and managing prime-hunting searches.

## Stack

| Technology | Version | Purpose |
|-----------|---------|---------|
| Next.js | 16.1.6 | React framework (static export mode) |
| React | 19.2.3 | UI component library |
| Tailwind CSS | 4 | Utility-first styling |
| shadcn/ui | 3.8.5 | Copy-paste component system (Radix UI primitives) |
| Recharts | 3.7.0 | Charts (discovery timeline, digit distribution, throughput) |
| Lucide React | 0.574.0 | Icon library |
| Sonner | 2.0.7 | Toast notifications |
| @supabase/supabase-js | ^2 | Supabase client (auth, queries, realtime) |
| react-markdown | 10.1.0 | Markdown rendering for docs |
| remark-math + rehype-katex | 6.0.0 / 7.0.1 | LaTeX math rendering in docs |

## Build & Dev

```bash
cd frontend
npm install
npm run dev      # Development server (hot reload)
npm run build    # Static export to frontend/out/
```

The static export is served by the Rust backend (`dashboard.rs` serves `--static-dir`). No Node.js server needed at runtime.

## Data Flow

```
Frontend (Next.js static export)
├── Supabase Auth (login, session management)
├── Supabase JS client (read primes, stats, charts)
│   ├── use-primes.ts     → supabase.from("primes") queries
│   ├── use-stats.ts      → supabase.rpc("get_stats")
│   ├── use-timeline.ts   → supabase.rpc("get_discovery_timeline")
│   └── use-distribution.ts → supabase.rpc("get_digit_distribution")
├── Supabase Realtime (live prime notifications)
│   └── use-prime-realtime.ts → postgres_changes INSERT on primes
└── WebSocket → Rust backend (coordination only)
    └── use-websocket.ts → fleet, searches, deployments, status
```

**Prime data** comes from Supabase directly. The WebSocket is used only for coordination data (fleet status, search management, deployments).

## Directory Structure

```
src/
├── app/
│   ├── page.tsx           # Main dashboard (stat cards, primes table, charts)
│   ├── layout.tsx         # Root layout, AuthProvider, WebSocketProvider
│   ├── globals.css        # Tailwind globals
│   ├── login/
│   │   └── page.tsx       # Email/password login (Supabase Auth)
│   ├── browse/
│   │   └── page.tsx       # Primes browser (filtering, pagination, detail dialog)
│   ├── searches/
│   │   └── page.tsx       # Search management
│   ├── fleet/
│   │   └── page.tsx       # Fleet monitoring
│   └── docs/
│       └── page.tsx       # Documentation viewer (search, markdown + KaTeX)
├── components/
│   ├── ui/                # shadcn components
│   ├── charts/            # Recharts visualizations
│   │   ├── discovery-timeline.tsx
│   │   ├── digit-distribution.tsx
│   │   └── throughput-gauge.tsx
│   ├── prime-notifier.tsx  # Supabase Realtime toast notifications
│   └── app-header.tsx      # Navigation header
├── contexts/
│   ├── auth-context.tsx    # Supabase Auth (AuthProvider, AuthGuard, useAuth)
│   └── websocket-context.tsx # WebSocket context (coordination data only)
├── hooks/
│   ├── use-websocket.ts    # WebSocket client (fleet, searches, deployments, status)
│   ├── use-primes.ts       # Supabase primes queries (PrimeRecord, PrimeFilter, PrimeDetail)
│   ├── use-stats.ts        # Supabase stats RPC
│   ├── use-timeline.ts     # Supabase discovery timeline RPC
│   ├── use-distribution.ts # Supabase digit distribution RPC
│   ├── use-prime-realtime.ts # Supabase Realtime INSERT subscription
│   └── use-theme.ts        # Dark/light theme toggle (localStorage)
└── lib/
    ├── supabase.ts         # Supabase client singleton
    ├── format.ts           # Formatting utilities
    └── utils.ts            # Utility functions (cn class merger)
```

## Conventions

- **Components**: Use shadcn/ui components from `components/ui/`. Add new ones via `npx shadcn@latest add <component>`.
- **Styling**: Tailwind utility classes only. No custom CSS except in `globals.css`.
- **State**: Supabase hooks for prime data; WebSocket for coordination data. Local state for UI interactions only.
- **Types**: TypeScript strict mode. Prime types in `use-primes.ts`, coordination types in `use-websocket.ts`, chart types in their respective hooks.
- **Auth**: Supabase Auth with email/password. `AuthGuard` in layout gates all pages except `/login`.
- **Icons**: Use Lucide React (`import { IconName } from "lucide-react"`).
- **Charts**: Recharts with responsive containers. Match dark theme colors.

## Adding a New UI Component

```bash
npx shadcn@latest add <component-name>
# Example: npx shadcn@latest add tabs
```

This copies the component source into `components/ui/`. Components are fully owned and customizable.

## Roadmap

See `docs/roadmaps/frontend.md` for planned features: filtering, search management, visualization improvements, multi-node coordination UI.
