/**
 * @file Tests for the HostNodeCard component
 * @module __tests__/components/host-node-card
 *
 * Validates the server node card displayed on the Network page. Each card
 * represents a physical or virtual host running darkreach workers. Tests
 * cover hostname rendering, core count (singular/plural), coordinator badge,
 * hardware metrics bars (CPU/MEM/Disk), load averages, worker process rows,
 * worker click inspection callback, deployment section rendering, and
 * conditional section visibility.
 *
 * @see {@link ../../components/host-node-card} Source component (HostNodeCard, HostNode type)
 * @see {@link ../../hooks/use-websocket} HardwareMetrics, WorkerStatus, Deployment types
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock MetricsBar — renders as a simple div with label, percent, and detail.
vi.mock("@/components/metrics-bar", () => ({
  MetricsBar: ({
    label,
    percent,
    detail,
  }: {
    label: string;
    percent: number;
    detail?: string;
  }) => (
    <div data-testid={`metrics-bar-${label.toLowerCase()}`}>
      {label}: {percent}%{detail ? ` (${detail})` : ""}
    </div>
  ),
}));

// Mock ProcessRow
vi.mock("@/components/process-row", () => ({
  ProcessRow: ({
    worker,
  }: {
    worker: { worker_id: string };
  }) => (
    <div data-testid={`process-row-${worker.worker_id}`}>
      {worker.worker_id}
    </div>
  ),
}));

import { HostNodeCard, type HostNode } from "@/components/host-node-card";
import type { HardwareMetrics, WorkerStatus, ManagedSearch, Deployment } from "@/hooks/use-websocket";

/** Factory helper — creates HardwareMetrics with realistic default values. */
function makeMetrics(overrides: Partial<HardwareMetrics> = {}): HardwareMetrics {
  return {
    cpu_usage_percent: 45.0,
    memory_used_gb: 8.0,
    memory_total_gb: 16.0,
    memory_usage_percent: 50.0,
    disk_used_gb: 100.0,
    disk_total_gb: 500.0,
    disk_usage_percent: 20.0,
    load_avg_1m: 2.0,
    load_avg_5m: 1.8,
    load_avg_15m: 1.5,
    ...overrides,
  };
}

/** Factory helper — creates a WorkerStatus with realistic default values. */
function makeWorker(overrides: Partial<WorkerStatus> = {}): WorkerStatus {
  return {
    worker_id: "worker-1",
    hostname: "compute-01",
    cores: 8,
    search_type: "factorial",
    search_params: "{}",
    current: "5000!+1",
    tested: 1234,
    found: 2,
    uptime_secs: 3600,
    last_heartbeat_secs_ago: 5,
    ...overrides,
  };
}

/** Factory helper — creates a HostNode with empty workers/metrics by default. */
function makeNode(overrides: Partial<HostNode> = {}): HostNode {
  return {
    hostname: "compute-01",
    isCoordinator: false,
    metrics: null,
    workers: [],
    searches: [],
    deployments: [],
    totalCores: 8,
    totalTested: 5000,
    totalFound: 10,
    ...overrides,
  };
}

// Tests the HostNodeCard: hostname, core count, coordinator badge, metrics bars,
// load averages, worker process rows, click callbacks, and deployment sections.
describe("HostNodeCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /** Verifies the node's hostname displays as the card title. */
  it("renders hostname", () => {
    render(<HostNodeCard node={makeNode({ hostname: "server-alpha" })} />);
    expect(screen.getByText("server-alpha")).toBeInTheDocument();
  });

  /** Verifies the total core count renders with plural "cores" label. */
  it("renders core count", () => {
    render(<HostNodeCard node={makeNode({ totalCores: 16 })} />);
    expect(screen.getByText("16 cores")).toBeInTheDocument();
  });

  /** Verifies singular "core" label for single-core nodes. */
  it("renders singular core label", () => {
    render(<HostNodeCard node={makeNode({ totalCores: 1 })} />);
    expect(screen.getByText("1 core")).toBeInTheDocument();
  });

  /** Verifies the "Coordinator" badge appears for the coordinator node. */
  it("shows Coordinator badge when isCoordinator", () => {
    render(<HostNodeCard node={makeNode({ isCoordinator: true })} />);
    expect(screen.getByText("Coordinator")).toBeInTheDocument();
  });

  /** Verifies the "Coordinator" badge is absent for regular worker nodes. */
  it("hides Coordinator badge for non-coordinators", () => {
    render(<HostNodeCard node={makeNode({ isCoordinator: false })} />);
    expect(screen.queryByText("Coordinator")).not.toBeInTheDocument();
  });

  /** Verifies CPU, MEM, and Disk metrics bars render when hardware data exists. */
  it("renders hardware metrics when present", () => {
    render(
      <HostNodeCard node={makeNode({ metrics: makeMetrics() })} />
    );
    expect(screen.getByTestId("metrics-bar-cpu")).toBeInTheDocument();
    expect(screen.getByTestId("metrics-bar-mem")).toBeInTheDocument();
    expect(screen.getByTestId("metrics-bar-disk")).toBeInTheDocument();
  });

  /** Verifies the 1/5/15-minute load average display format. */
  it("renders load averages when metrics present", () => {
    render(
      <HostNodeCard
        node={makeNode({
          metrics: makeMetrics({
            load_avg_1m: 3.2,
            load_avg_5m: 2.1,
            load_avg_15m: 1.9,
          }),
        })}
      />
    );
    expect(screen.getByText("Load: 3.2 / 2.1 / 1.9")).toBeInTheDocument();
  });

  /** Verifies the metrics section is hidden when no hardware data is available. */
  it("does not render metrics section when metrics is null", () => {
    render(<HostNodeCard node={makeNode({ metrics: null })} />);
    expect(
      screen.queryByTestId("metrics-bar-cpu")
    ).not.toBeInTheDocument();
  });

  /** Verifies the "Processes" section and worker rows appear when workers exist. */
  it("renders worker processes section", () => {
    render(
      <HostNodeCard
        node={makeNode({
          workers: [makeWorker({ worker_id: "w-abc" })],
        })}
      />
    );
    expect(screen.getByText("Processes")).toBeInTheDocument();
    expect(screen.getByTestId("process-row-w-abc")).toBeInTheDocument();
  });

  /** Verifies the "Processes" section is hidden when the node has no workers. */
  it("hides processes section when no workers", () => {
    render(<HostNodeCard node={makeNode({ workers: [] })} />);
    expect(screen.queryByText("Processes")).not.toBeInTheDocument();
  });

  /** Verifies that clicking a worker row fires the onInspectWorker callback with the worker. */
  it("calls onInspectWorker when worker row is clicked", () => {
    const onInspect = vi.fn();
    const worker = makeWorker({ worker_id: "w-click" });
    render(
      <HostNodeCard
        node={makeNode({ workers: [worker] })}
        onInspectWorker={onInspect}
      />
    );
    fireEvent.click(screen.getByTestId("process-row-w-click"));
    expect(onInspect).toHaveBeenCalledWith(worker);
  });

  /** Verifies the "Deployments" section renders with SSH connection info. */
  it("renders deployments section when deployments exist", () => {
    const deployment: Deployment = {
      id: 1,
      hostname: "compute-01",
      ssh_user: "root",
      search_type: "kbn",
      search_params: "{}",
      worker_id: "w-1",
      status: "running",
      error: null,
      remote_pid: 1234,
      started_at: "2026-01-15T10:00:00Z",
    };
    render(
      <HostNodeCard
        node={makeNode({ deployments: [deployment] })}
      />
    );
    expect(screen.getByText("Deployments")).toBeInTheDocument();
    expect(screen.getByText("root@compute-01")).toBeInTheDocument();
  });

  /** Verifies the "Deployments" section is hidden when there are no deployments. */
  it("does not render deployments section when none exist", () => {
    render(<HostNodeCard node={makeNode({ deployments: [] })} />);
    expect(screen.queryByText("Deployments")).not.toBeInTheDocument();
  });
});
