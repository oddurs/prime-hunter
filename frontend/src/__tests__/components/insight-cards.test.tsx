/**
 * @file Tests for the InsightCards analytics component
 * @module __tests__/components/insight-cards
 *
 * Validates the four insight summary cards shown on the dashboard:
 * Discovery Rate, Last Discovery, Fleet Throughput, and Record Prime.
 * Tests cover icon rendering, rate computation from timeline data
 * (discoveries/day over 7-day window), relative timestamp for last
 * discovery, null/missing data fallbacks, fleet throughput initial state,
 * record prime expression and digit formatting, and the 7-day trend count.
 *
 * @see {@link ../../components/insight-cards} Source component
 * @see {@link ../../hooks/use-timeline} TimelineBucket type (discovery rate)
 * @see {@link ../../hooks/use-stats} Stats type (record prime)
 * @see {@link ../../hooks/use-websocket} FleetData type (throughput)
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock lucide-react icons used in the four insight cards.
vi.mock("lucide-react", () => ({
  TrendingUp: () => <span data-testid="trending-up" />,
  TrendingDown: () => <span data-testid="trending-down" />,
  Clock: () => <span data-testid="clock" />,
  Gauge: () => <span data-testid="gauge" />,
  Trophy: () => <span data-testid="trophy" />,
}));

// Mock recharts to avoid rendering issues in tests
vi.mock("recharts", () => ({
  LineChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="line-chart">{children}</div>
  ),
  Line: () => null,
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="responsive-container">{children}</div>
  ),
}));

// Mock format module
vi.mock("@/lib/format", () => ({
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  relativeTime: (iso: string) => "3h ago",
  formLabels: {
    factorial: "Factorial",
  } as Record<string, string>,
}));

import { InsightCards } from "@/components/insight-cards";
import type { TimelineBucket } from "@/hooks/use-timeline";
import type { Stats } from "@/hooks/use-stats";
import type { FleetData } from "@/hooks/use-websocket";
import type { PrimeRecord } from "@/hooks/use-primes";

function makeTimelineBucket(daysAgo: number, count: number): TimelineBucket {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  return { bucket: d.toISOString(), count };
}

const defaultStats: Stats = {
  total: 100,
  by_form: [{ form: "factorial", count: 100 }],
  largest_digits: 45678,
  largest_expression: "99999!+1",
};

const defaultFleet: FleetData = {
  workers: [],
  total_workers: 4,
  total_cores: 32,
  total_tested: 50000,
  total_found: 10,
};

const defaultPrime: PrimeRecord = {
  id: 1,
  form: "factorial",
  expression: "100!+1",
  digits: 158,
  found_at: "2026-01-15T10:30:00Z",
  proof_method: "deterministic",
  verified: false,
  verified_at: null,
  verification_method: null,
  verification_tier: null,
};

// Tests the InsightCards component: four summary cards with icons, computed rates,
// timestamps, fleet throughput, record prime info, and null data fallbacks.
describe("InsightCards", () => {
  /** Verifies all four insight card titles are present. */
  it("renders all four card titles", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByText("Discovery Rate")).toBeInTheDocument();
    expect(screen.getByText("Last Discovery")).toBeInTheDocument();
    expect(screen.getByText("Fleet Throughput")).toBeInTheDocument();
    expect(screen.getByText("Record Prime")).toBeInTheDocument();
  });

  /** Verifies each card renders its corresponding Lucide icon. */
  it("renders all four icons", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByTestId("trending-up")).toBeInTheDocument();
    expect(screen.getByTestId("clock")).toBeInTheDocument();
    expect(screen.getByTestId("gauge")).toBeInTheDocument();
    expect(screen.getByTestId("trophy")).toBeInTheDocument();
  });

  /** Verifies the discovery rate shows "0.0/day" when no timeline data is provided. */
  it("renders discovery rate as 0.0/day when no timeline data", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByText("0.0/day")).toBeInTheDocument();
  });

  /** Verifies the rate computation: sum of last 7 days / 7 = discoveries per day. */
  it("computes discovery rate from recent timeline buckets", () => {
    const timeline = [
      makeTimelineBucket(1, 5),
      makeTimelineBucket(2, 3),
      makeTimelineBucket(3, 2),
    ];
    render(
      <InsightCards
        timeline={timeline}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    // Rate = (5+3+2)/7 = 1.4/day
    expect(screen.getByText("1.4/day")).toBeInTheDocument();
  });

  /** Verifies the "Last Discovery" card shows a relative time string. */
  it("renders last discovery relative time", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByText("3h ago")).toBeInTheDocument();
  });

  /** Verifies the "-" fallback when no primes have been discovered yet. */
  it("renders '-' for last discovery when no prime", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={null}
      />
    );
    // The last discovery card should show "-"
    const cards = screen.getAllByText("-");
    expect(cards.length).toBeGreaterThan(0);
  });

  /** Verifies the latest prime's expression shows below the Last Discovery card. */
  it("renders latest prime expression", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByText("100!+1")).toBeInTheDocument();
  });

  /** Verifies the record prime digit count uses thousand separators. */
  it("renders record prime digits with commas", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={{ ...defaultStats, largest_digits: 45678 }}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByText("45,678 digits")).toBeInTheDocument();
  });

  /** Verifies the "-" fallback when stats data is not yet loaded. */
  it("renders '-' for record prime when stats is null", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={null}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    const dashes = screen.getAllByText("-");
    expect(dashes.length).toBeGreaterThan(0);
  });

  /** Verifies the record prime expression renders below the Record Prime card. */
  it("renders largest expression when available", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={{
          ...defaultStats,
          largest_expression: "99999!+1",
        }}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByText("99999!+1")).toBeInTheDocument();
  });

  /** Verifies the Fleet Throughput card shows "0/s" initially (no throughput data). */
  it("renders fleet throughput initial state as 0/s", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByText("0/s")).toBeInTheDocument();
  });

  /** Verifies the 7-day trend count shows "0 in last 7d" when timeline is empty. */
  it("renders 0 in last 7d when no trend data and zero rate", () => {
    render(
      <InsightCards
        timeline={[]}
        stats={defaultStats}
        fleet={defaultFleet}
        latestPrime={defaultPrime}
      />
    );
    expect(screen.getByText("0 in last 7d")).toBeInTheDocument();
  });
});
