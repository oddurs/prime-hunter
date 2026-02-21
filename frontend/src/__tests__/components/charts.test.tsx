/**
 * @file Tests for Recharts visualization components
 * @module __tests__/components/charts
 *
 * Validates the three chart components used throughout the dashboard:
 * - DiscoveryTimeline: stacked area chart of prime discoveries over time
 * - DigitDistribution: bar chart histogram of prime digit counts
 * - ThroughputGauge: candidates/sec metric display based on fleet data
 *
 * Recharts is mocked to simple div elements since jsdom cannot render SVG.
 * Tests focus on conditional rendering (empty vs populated data), correct
 * chart element creation per data series, and proper section titles.
 *
 * @see {@link ../../components/charts/discovery-timeline} DiscoveryTimeline source
 * @see {@link ../../components/charts/digit-distribution} DigitDistribution source
 * @see {@link ../../components/charts/throughput-gauge} ThroughputGauge source
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { DiscoveryTimeline } from "@/components/charts/discovery-timeline";
import { DigitDistribution } from "@/components/charts/digit-distribution";
import { ThroughputGauge } from "@/components/charts/throughput-gauge";
import type { TimelineBucket } from "@/hooks/use-timeline";
import type { DigitBucket } from "@/hooks/use-distribution";
import type { FleetData } from "@/hooks/use-websocket";

// Mock Recharts to avoid SVG rendering issues in jsdom — replaces chart
// components with simple divs carrying data-testid attributes.
vi.mock("recharts", () => ({
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="responsive-container">{children}</div>
  ),
  AreaChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="area-chart">{children}</div>
  ),
  Area: () => <div data-testid="area" />,
  BarChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="bar-chart">{children}</div>
  ),
  Bar: () => <div data-testid="bar" />,
  LineChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="line-chart">{children}</div>
  ),
  Line: () => <div data-testid="line" />,
  XAxis: () => <div data-testid="x-axis" />,
  YAxis: () => <div data-testid="y-axis" />,
  Tooltip: () => <div data-testid="tooltip" />,
}));

// Tests the DiscoveryTimeline: a stacked area chart showing prime discovery
// counts per form over time. Data comes from the use-timeline Supabase RPC hook.
describe("DiscoveryTimeline", () => {
  /** Verifies the component returns an empty container when there are no data points. */
  it("renders nothing with empty data", () => {
    const { container } = render(<DiscoveryTimeline data={[]} />);
    expect(container.innerHTML).toBe("");
  });

  it("renders chart with data", () => {
    const data: TimelineBucket[] = [
      { bucket: "2026-01-01", form: "factorial", count: 5 },
      { bucket: "2026-01-02", form: "factorial", count: 3 },
      { bucket: "2026-01-01", form: "kbn", count: 2 },
    ];
    render(<DiscoveryTimeline data={data} />);
    expect(screen.getByText("Discovery timeline")).toBeInTheDocument();
    expect(screen.getByTestId("area-chart")).toBeInTheDocument();
  });

  /** Verifies that one Area element is created per unique prime form in the data. */
  it("creates areas for each form", () => {
    const data: TimelineBucket[] = [
      { bucket: "2026-01-01", form: "factorial", count: 5 },
      { bucket: "2026-01-01", form: "kbn", count: 2 },
    ];
    render(<DiscoveryTimeline data={data} />);
    const areas = screen.getAllByTestId("area");
    expect(areas).toHaveLength(2); // factorial + kbn
  });
});

// Tests the DigitDistribution: a grouped bar chart showing how many primes
// fall into each digit-count bucket, broken down by form.
describe("DigitDistribution", () => {
  /** Verifies the component returns an empty container when there are no buckets. */
  it("renders nothing with empty data", () => {
    const { container } = render(<DigitDistribution data={[]} />);
    expect(container.innerHTML).toBe("");
  });

  /** Verifies the chart title and bar chart render when bucket data is provided. */
  /** Verifies the chart title and bar chart render when bucket data is provided. */
  it("renders chart with data", () => {
    const data: DigitBucket[] = [
      { bucket_start: 0, form: "factorial", count: 10 },
      { bucket_start: 10, form: "factorial", count: 5 },
    ];
    render(<DigitDistribution data={data} />);
    expect(screen.getByText("Digit distribution")).toBeInTheDocument();
    expect(screen.getByTestId("bar-chart")).toBeInTheDocument();
  });

  /** Verifies that one Bar element is created per unique prime form in the data. */
  it("creates bars for each form", () => {
    const data: DigitBucket[] = [
      { bucket_start: 0, form: "factorial", count: 10 },
      { bucket_start: 0, form: "kbn", count: 3 },
    ];
    render(<DigitDistribution data={data} />);
    const bars = screen.getAllByTestId("bar");
    expect(bars).toHaveLength(2);
  });
});

// Tests the ThroughputGauge: displays aggregate candidates/sec throughput
// computed from fleet worker data received via the WebSocket.
describe("ThroughputGauge", () => {
  /** Verifies the gauge hides entirely when no workers are connected. */
  it("renders nothing when no workers", () => {
    const fleet: FleetData = {
      workers: [],
      total_workers: 0,
      total_cores: 0,
      total_tested: 0,
      total_found: 0,
    };
    const { container } = render(<ThroughputGauge fleet={fleet} />);
    expect(container.innerHTML).toBe("");
  });

  /** Verifies the gauge renders with title and unit label when workers are active. */
  it("renders gauge when workers present", () => {
    const fleet: FleetData = {
      workers: [{ worker_id: "w1", hostname: "host", cores: 8, search_type: "kbn", search_params: "{}", current: "", tested: 100, found: 1, uptime_secs: 60, last_heartbeat_secs_ago: 1 }],
      total_workers: 1,
      total_cores: 8,
      total_tested: 100,
      total_found: 1,
    };
    render(<ThroughputGauge fleet={fleet} />);
    expect(screen.getByText("Search throughput")).toBeInTheDocument();
    expect(screen.getByText("candidates/sec")).toBeInTheDocument();
  });
});
