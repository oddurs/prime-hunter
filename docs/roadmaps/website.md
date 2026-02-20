# Website Roadmap

## Current State (February 2026)

The darkreach website (`website/`) is a multi-route Next.js 16 static export deployed to Vercel. It serves as the public face of the project across three domains:

- **darkreach.ai** — Landing page, about, blog, download, leaderboard
- **docs.darkreach.ai** — Documentation hub (5 pages + sidebar nav)
- **status.darkreach.ai** — Service health and fleet overview

### What's Built

| Route | Status | Description |
|-------|--------|-------------|
| `/` | Complete | Homepage with AI narrative, stats, feature grid, pipeline, 12 prime forms, discoveries, comparison, CTA |
| `/about` | Complete | Mission, timeline, tech stack, open source callout |
| `/blog` | Complete | Blog index with 5 mocked posts |
| `/download` | Complete | OS detection, tabbed install methods, system requirements |
| `/download/server` | Complete | Coordinator setup guide with systemd config |
| `/download/worker` | Complete | Worker deployment guide with scaling instructions |
| `/docs` | Complete | Quick-links grid hub |
| `/docs/getting-started` | Complete | Prerequisites → build → first search → checkpointing |
| `/docs/architecture` | Complete | System diagram, engine/server/frontend breakdown |
| `/docs/prime-forms` | Complete | All 12 forms with OEIS refs, algorithms, CLI commands |
| `/docs/api` | Complete | REST endpoints + WebSocket events (mocked reference) |
| `/docs/contributing` | Complete | Fork/PR workflow, code style, adding new forms |
| `/status` | Complete | Service cards, fleet stats, 90-day uptime bars, incidents |
| `/leaderboard` | Complete | Individual + team rankings (mocked data) |

### Architecture

- **Framework**: Next.js 16 with `output: "export"` (pure static)
- **Styling**: Tailwind CSS 4 with inline `@theme` custom properties
- **Icons**: lucide-react
- **Utilities**: clsx for class composition
- **Subdomain routing**: Vercel `rewrites` in `vercel.json` with host-based matching
- **No backend integration**: All data is hardcoded/mocked

### Design System

CSS custom properties aligned with the dashboard (`frontend/`):
- Colors: GitHub-style dark mode (#0d1117 bg, #161b22 secondary, #30363d borders)
- Brand purple: #bc8cff (logo, accents)
- Primary blue: #2f81f7 (links, interactive)
- System fonts: -apple-system stack (matches dashboard)

### Component Library

Lightweight UI primitives (no shadcn — minimal deps):
- `ui/button.tsx` — Primary, outline, ghost variants
- `ui/card.tsx` — Standard card with optional hover glow
- `ui/badge.tsx` — Status tags with color variants
- `ui/code-block.tsx` — Syntax display with copy button
- `ui/section.tsx` — Consistent section wrapper

---

## Future Phases

### Phase A: Live Data Integration
- Connect stats bar to `api.darkreach.ai/api/stats` for real-time numbers
- Connect status page to `api.darkreach.ai/api/status` for live health checks
- Graceful fallback to static values when API is unreachable
- Connect leaderboard to real contributor data from PostgreSQL

### Phase B: MDX Blog
- Replace mocked blog cards with MDX-based content
- Individual post pages (`/blog/[slug]`)
- Syntax highlighting with shiki or rehype-pretty-code
- RSS feed generation
- Author profiles

### Phase C: Documentation Enhancements
- Full-text search across docs (Pagefind or Algolia DocSearch)
- Table of contents sidebar for long pages
- Previous/Next navigation between doc pages
- Code copy buttons on all code blocks in prose
- Version selector (if multiple darkreach versions are maintained)

### Phase D: Interactive Features
- Dark/light mode toggle (currently dark-only, matching dashboard)
- Newsletter signup (email collection)
- Discord widget integration
- Real-time prime notification toast on homepage
- Interactive prime form explorer (try parameters, see sample output)

### Phase E: Performance & SEO
- Lighthouse audit and optimization (target 95+ on all metrics)
- Structured data (JSON-LD) for search engines
- Sitemap generation
- Image optimization (OG images for social sharing)
- Analytics integration (Plausible or self-hosted)

### Phase F: Internationalization
- i18n support for docs (priority: English, Chinese, Japanese)
- Locale-aware routing
- Translation workflow with community contributions

---

## Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Component library | Custom UI primitives | Keeps deps minimal; shadcn would add 10+ deps for 5 components |
| Subdomain routing | Vercel rewrites (host-based) | Single project, single deploy, no server needed |
| Content management | Hardcoded TSX | Simple for v1; migrate to MDX when blog grows |
| Styling | Tailwind CSS 4 + CSS custom properties | Matches dashboard, utility-first, no runtime cost |
| Icons | lucide-react | Same library as dashboard, tree-shakeable |
| State management | None (static export) | No client state beyond OS detection and mobile nav toggle |
| Data fetching | None (mocked) | Will add client-side fetch with SWR/React Query for live data |

---

## File Structure

```
website/
├── src/
│   ├── app/
│   │   ├── layout.tsx              # Shared navbar + footer
│   │   ├── page.tsx                # Homepage
│   │   ├── globals.css             # Design tokens
│   │   ├── about/page.tsx
│   │   ├── blog/page.tsx
│   │   ├── download/
│   │   │   ├── page.tsx            # Main download page
│   │   │   ├── server/page.tsx     # Coordinator guide
│   │   │   └── worker/page.tsx     # Worker guide
│   │   ├── docs/
│   │   │   ├── layout.tsx          # Sidebar layout
│   │   │   ├── page.tsx            # Docs hub
│   │   │   ├── getting-started/
│   │   │   ├── architecture/
│   │   │   ├── prime-forms/
│   │   │   ├── api/
│   │   │   └── contributing/
│   │   ├── status/page.tsx
│   │   └── leaderboard/page.tsx
│   ├── components/
│   │   ├── ui/                     # Primitives (button, card, badge, etc.)
│   │   ├── navbar.tsx              # Multi-route nav with dropdowns
│   │   ├── footer.tsx              # 4-column footer
│   │   ├── mobile-nav.tsx          # Mobile overlay nav
│   │   ├── hero.tsx                # AI narrative hero
│   │   ├── feature-grid.tsx        # 3-col AI capabilities
│   │   ├── pipeline.tsx            # 4-step pipeline visualization
│   │   ├── cta-section.tsx         # Two-path CTA (worker/server)
│   │   ├── ... (other section components)
│   │   ├── doc-sidebar.tsx         # Docs sidebar nav
│   │   ├── status-card.tsx         # Service health card
│   │   ├── uptime-bar.tsx          # 90-day uptime visualization
│   │   ├── timeline.tsx            # Vertical timeline
│   │   └── blog-card.tsx           # Blog post card
│   └── lib/
│       ├── cn.ts                   # clsx helper
│       ├── prime-forms.ts          # 12 form definitions
│       ├── docs-nav.ts             # Sidebar navigation structure
│       ├── install-commands.ts     # OS-specific install commands
│       ├── status-data.ts          # Mock status/fleet data
│       ├── leaderboard-data.ts     # Mock contributor rankings
│       └── blog-posts.ts           # Mock blog posts
├── vercel.json                     # Subdomain rewrites
├── next.config.ts                  # Static export config
└── package.json
```
