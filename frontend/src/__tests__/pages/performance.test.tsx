/**
 * @file Tests for the Performance (Observability) page
 * @module __tests__/pages/performance
 *
 * Validates the Performance page at `/performance`, which provides Grafana-style
 * metrics charts for throughput, worker count, and coordinator load over
 * configurable time ranges. Data is fetched from the REST API
 * `/api/observability/*`. Tests verify page heading, subtitle, time range
 * buttons (6h, 24h, 7d, 30d), refresh/export controls, and chart section titles.
 *
 * @see {@link ../../app/performance/page} Source page
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("@/contexts/websocket-context", () => ({
  useWs: () => ({
    fleet: {
      workers: [],
      total_workers: 0,
      total_cores: 0,
      total_tested: 0,
      total_found: 0,
    },
    coordinator: null,
    connected: true,
    searchJobs: [],
  }),
}));

vi.mock("@/components/view-header", () => ({
  ViewHeader: ({
    title,
    subtitle,
    actions,
    metadata,
  }: {
    title: string;
    subtitle: string;
    actions?: React.ReactNode;
    metadata?: React.ReactNode;
  }) => (
    <div data-testid="view-header">
      <h1>{title}</h1>
      <p>{subtitle}</p>
      {metadata}
      {actions}
    </div>
  ),
}));

vi.mock("@/components/stat-card", () => ({
  StatCard: ({ label, value }: { label: string; value: React.ReactNode }) => (
    <div data-testid={`stat-${label.toLowerCase().replace(/[^a-z0-9]/g, "-")}`}>
      {label}: {value}
    </div>
  ),
}));

vi.mock("recharts", () => ({
  LineChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="line-chart">{children}</div>
  ),
  Line: () => null,
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  XAxis: () => null,
  YAxis: () => null,
  Tooltip: () => null,
}));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  formatTime: (t: string) => t,
  numberWithCommas: (x: number) => String(x),
  relativeTime: (t: string) => t,
}));

const mockFetch = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  global.fetch = mockFetch;
  // Default: return empty series/logs/report for all observability fetches
  mockFetch.mockResolvedValue({
    ok: true,
    json: () =>
      Promise.resolve({
        series: [],
        logs: [],
        primes: { total: 0, by_form: [] },
        logs_summary: { by_level: [] },
        fleet: {},
        coordinator: {},
        workers: [],
        from: "",
        to: "",
      }),
  });
});

import PerformancePage from "@/app/performance/page";

// Tests the PerformancePage: heading, subtitle, time range buttons,
// refresh/export controls, and chart section titles.
describe("PerformancePage", () => {
  it("renders without crashing", () => {
    render(<PerformancePage />);
    expect(screen.getByText("Observability")).toBeInTheDocument();
  });

  it("shows subtitle", () => {
    render(<PerformancePage />);
    expect(
      screen.getByText("Grafana-style metrics, long-term trends, and system logs")
    ).toBeInTheDocument();
  });

  it("renders time range buttons", () => {
    render(<PerformancePage />);
    expect(screen.getByText("6h")).toBeInTheDocument();
    expect(screen.getByText("24h")).toBeInTheDocument();
    expect(screen.getByText("7d")).toBeInTheDocument();
    expect(screen.getByText("30d")).toBeInTheDocument();
  });

  it("renders Refresh and export buttons", () => {
    render(<PerformancePage />);
    expect(screen.getByText("Refresh")).toBeInTheDocument();
    expect(screen.getByText("Export Report")).toBeInTheDocument();
    expect(screen.getByText("Export CSV")).toBeInTheDocument();
  });

  it("renders chart section titles", () => {
    render(<PerformancePage />);
    expect(screen.getByText("Throughput (candidates/sec)")).toBeInTheDocument();
    expect(screen.getByText("Worker count")).toBeInTheDocument();
    expect(screen.getByText("Coordinator load")).toBeInTheDocument();
  });
});
