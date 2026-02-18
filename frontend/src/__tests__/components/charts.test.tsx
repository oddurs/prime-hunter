import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { DiscoveryTimeline } from "@/components/charts/discovery-timeline";
import { DigitDistribution } from "@/components/charts/digit-distribution";
import { ThroughputGauge } from "@/components/charts/throughput-gauge";
import type { TimelineBucket } from "@/hooks/use-timeline";
import type { DigitBucket } from "@/hooks/use-distribution";
import type { FleetData } from "@/hooks/use-websocket";

// Mock Recharts to avoid SVG rendering issues in jsdom
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

describe("DiscoveryTimeline", () => {
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

describe("DigitDistribution", () => {
  it("renders nothing with empty data", () => {
    const { container } = render(<DigitDistribution data={[]} />);
    expect(container.innerHTML).toBe("");
  });

  it("renders chart with data", () => {
    const data: DigitBucket[] = [
      { bucket_start: 0, form: "factorial", count: 10 },
      { bucket_start: 10, form: "factorial", count: 5 },
    ];
    render(<DigitDistribution data={data} />);
    expect(screen.getByText("Digit distribution")).toBeInTheDocument();
    expect(screen.getByTestId("bar-chart")).toBeInTheDocument();
  });

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

describe("ThroughputGauge", () => {
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
