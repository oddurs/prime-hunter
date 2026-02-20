"use client";

import { CodeBlock } from "@/components/ui/code-block";

export default function ArchitecturePage() {
  return (
    <div className="prose-docs">
      <h1>Architecture</h1>
      <p>
        darkreach is a three-tier system: a Rust engine for computation, an Axum
        server for coordination, and a Next.js frontend for monitoring and
        management.
      </p>

      <h2>System Overview</h2>
      <CodeBlock>
        {`┌─────────────────────────────────────────────────────┐
│                   darkreach.ai                      │
│              (Next.js static export)                │
│           Landing · Docs · Status · Blog            │
└─────────────────────────────────────────────────────┘

┌───────────────────┐     ┌───────────────────────────┐
│  app.darkreach.ai │────▶│    api.darkreach.ai       │
│  (Next.js + SPA)  │ WS  │  (Axum web server)        │
│  Dashboard · Fleet│◀────│  REST API · WebSocket      │
│  Searches · Browse│     │  Fleet coordination        │
└───────────────────┘     └──────────┬────────────────┘
                                     │
                          ┌──────────▼────────────────┐
                          │      PostgreSQL            │
                          │  (Supabase / self-hosted)  │
                          │  primes · workers · jobs   │
                          └──────────┬────────────────┘
                                     │
              ┌──────────────────────┼──────────────────────┐
              │                      │                      │
     ┌────────▼───────┐   ┌─────────▼──────┐   ┌──────────▼───────┐
     │   Worker 1     │   │   Worker 2     │   │   Worker N       │
     │  darkreach CLI │   │  darkreach CLI │   │   darkreach CLI  │
     │  sieve→test→   │   │  sieve→test→   │   │   sieve→test→    │
     │  prove→report  │   │  prove→report  │   │   prove→report   │
     └────────────────┘   └────────────────┘   └──────────────────┘`}
      </CodeBlock>

      <h2>Engine</h2>
      <p>
        The engine is the core Rust library implementing 12 prime search
        algorithms. Each form follows the same pipeline:
      </p>
      <ol>
        <li>
          <strong>Sieve</strong> — Eliminate composites using form-specific
          sieves (wheel factorization, BSGS, Pollard P-1)
        </li>
        <li>
          <strong>Test</strong> — Run Miller-Rabin pre-screening, then
          specialized primality tests (Proth, LLR, Pepin)
        </li>
        <li>
          <strong>Prove</strong> — Generate deterministic primality certificates
          (Pocklington, Morrison, BLS)
        </li>
        <li>
          <strong>Report</strong> — Log results to PostgreSQL with certificate
          data
        </li>
      </ol>

      <h3>Key engine modules</h3>
      <table>
        <thead>
          <tr>
            <th>Module</th>
            <th>Purpose</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td><code>src/sieve.rs</code></td>
            <td>Eratosthenes sieve, Montgomery multiplication, BitSieve, wheel factorization</td>
          </tr>
          <tr>
            <td><code>src/lib.rs</code></td>
            <td>Trial division, MR pre-screening, Frobenius test, small primes table</td>
          </tr>
          <tr>
            <td><code>src/proof.rs</code></td>
            <td>Pocklington (N-1), Morrison (N+1), BLS deterministic proofs</td>
          </tr>
          <tr>
            <td><code>src/p1.rs</code></td>
            <td>Pollard P-1 factoring for deep composite elimination</td>
          </tr>
          <tr>
            <td><code>src/pfgw.rs</code></td>
            <td>PFGW subprocess integration (50-100x acceleration)</td>
          </tr>
          <tr>
            <td><code>src/certificate.rs</code></td>
            <td>PrimalityCertificate enum for witness serialization</td>
          </tr>
        </tbody>
      </table>

      <h2>Server</h2>
      <p>
        The server is an Axum web application that runs on the coordinator node.
        It provides:
      </p>
      <ul>
        <li>REST API for primes, workers, jobs, and projects</li>
        <li>WebSocket for real-time fleet coordination and prime notifications</li>
        <li>PostgreSQL-based work distribution with <code>FOR UPDATE SKIP LOCKED</code></li>
        <li>Checkpoint management for fault-tolerant search resumption</li>
      </ul>

      <h3>Key server modules</h3>
      <table>
        <thead>
          <tr>
            <th>Module</th>
            <th>Purpose</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td><code>src/dashboard.rs</code></td>
            <td>Axum routes, WebSocket handler, static file serving</td>
          </tr>
          <tr>
            <td><code>src/db/</code></td>
            <td>PostgreSQL queries (modular: workers, jobs, projects, primes)</td>
          </tr>
          <tr>
            <td><code>src/pg_worker.rs</code></td>
            <td>PostgreSQL-based work claiming with row-level locking</td>
          </tr>
          <tr>
            <td><code>src/search_manager.rs</code></td>
            <td>Search job lifecycle, block generation, work distribution</td>
          </tr>
          <tr>
            <td><code>src/worker_client.rs</code></td>
            <td>HTTP client for worker-to-coordinator communication</td>
          </tr>
        </tbody>
      </table>

      <h2>Frontend</h2>
      <p>
        Two separate Next.js applications:
      </p>
      <ul>
        <li>
          <strong>Website</strong> (<code>website/</code>) — Landing page, docs,
          status page. Static export deployed to Vercel.
        </li>
        <li>
          <strong>Dashboard</strong> (<code>frontend/</code>) — Real-time
          dashboard with Supabase for data and auth, WebSocket for fleet
          coordination.
        </li>
      </ul>

      <h2>Data Flow</h2>
      <ol>
        <li>Coordinator generates work blocks and inserts them into PostgreSQL</li>
        <li>Workers claim blocks using <code>FOR UPDATE SKIP LOCKED</code></li>
        <li>Workers run the sieve → test → prove pipeline</li>
        <li>Results (primes + certificates) are written back to PostgreSQL</li>
        <li>Dashboard queries PostgreSQL via Supabase for display</li>
        <li>WebSocket pushes real-time notifications for new primes</li>
      </ol>
    </div>
  );
}
