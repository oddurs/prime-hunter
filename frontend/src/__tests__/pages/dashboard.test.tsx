/**
 * @file Tests for the Dashboard page (root page)
 * @module __tests__/pages/dashboard
 *
 * Validates the main Dashboard page at `/`, which is the primary landing
 * page of the darkreach application. The dashboard aggregates data from
 * multiple sources: WebSocket (fleet, searches, coordinator status),
 * Supabase (stats, primes, timeline, distribution, leaderboard). Tests
 * verify heading/subtitle, current status section, idle state, infrastructure
 * section, insight cards, primes table, service status cards (Coordinator,
 * Database), agent controller, fleet stats strip, database metrics, action
 * buttons (New Search, Add Server), charts, form leaderboard, and activity feed.
 *
 * @see {@link ../../app/page} Source page
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock all hooks used by the Dashboard page — WebSocket context, Supabase hooks,
// and child components are replaced with simple mocks to isolate page logic.

vi.mock("@/contexts/websocket-context", () => ({
  useWs: () => ({
    status: { active: false, checkpoint: null },
    fleet: {
      workers: [],
      total_workers: 0,
      total_cores: 0,
      total_tested: 0,
      total_found: 0,
    },
    coordinator: null,
    searches: [],
    deployments: [],
    agentTasks: [],
    agentBudgets: [],
    runningAgents: [],
    connected: true,
  }),
}));

vi.mock("@/hooks/use-stats", () => ({
  useStats: () => ({
    stats: {
      total: 42,
      by_form: [
        { form: "factorial", count: 30 },
        { form: "kbn", count: 12 },
      ],
      largest_digits: 1000,
      largest_expression: "100!+1",
    },
  }),
}));

vi.mock("@/hooks/use-primes", () => ({
  usePrimes: () => ({
    primes: {
      primes: [
        {
          id: 1,
          form: "factorial",
          expression: "5!+1",
          digits: 3,
          found_at: "2026-01-01T00:00:00Z",
          proof_method: "deterministic",
          verified: false,
        },
      ],
      total: 1,
    },
    fetchPrimes: vi.fn(),
    selectedPrime: null,
    clearSelectedPrime: vi.fn(),
  }),
}));

vi.mock("@/hooks/use-timeline", () => ({
  useTimeline: () => ({
    timeline: [],
  }),
}));

vi.mock("@/hooks/use-distribution", () => ({
  useDistribution: () => ({
    distribution: [],
  }),
}));

vi.mock("@/hooks/use-form-leaderboard", () => ({
  useFormLeaderboard: () => ({
    entries: [],
    refetch: vi.fn(),
  }),
}));

// Mock child components that are complex or have their own dependencies
vi.mock("@/components/charts/discovery-timeline", () => ({
  DiscoveryTimeline: () => <div data-testid="discovery-timeline">Timeline</div>,
}));

vi.mock("@/components/charts/digit-distribution", () => ({
  DigitDistribution: () => <div data-testid="digit-distribution">Distribution</div>,
}));

vi.mock("@/components/new-search-dialog", () => ({
  NewSearchDialog: () => null,
}));

vi.mock("@/components/add-server-dialog", () => ({
  AddServerDialog: () => null,
}));

vi.mock("@/components/insight-cards", () => ({
  InsightCards: () => <div data-testid="insight-cards">Insight Cards</div>,
}));

vi.mock("@/components/form-leaderboard", () => ({
  FormLeaderboard: () => <div data-testid="form-leaderboard">Leaderboard</div>,
}));

vi.mock("@/components/activity-feed", () => ({
  ActivityFeed: () => <div data-testid="activity-feed">Activity</div>,
}));

vi.mock("@/components/host-node-card", () => ({
  HostNodeCard: () => <div>HostNode</div>,
}));

vi.mock("@/components/service-status-card", () => ({
  ServiceStatusCard: ({ name, children }: { name: string; children?: React.ReactNode }) => (
    <div data-testid={`service-${name.toLowerCase()}`}>
      {name}
      {children}
    </div>
  ),
}));

vi.mock("@/components/agent-controller-card", () => ({
  AgentControllerCard: () => <div data-testid="agent-controller">Agent Controller</div>,
}));

vi.mock("@/components/metrics-bar", () => ({
  MetricsBar: () => <div>MetricsBar</div>,
}));

vi.mock("@/components/worker-detail-dialog", () => ({
  WorkerDetailDialog: () => null,
}));

vi.mock("@/components/primes-table", () => ({
  PrimesTable: () => <div data-testid="primes-table">Primes Table</div>,
}));

vi.mock("@/components/view-header", () => ({
  ViewHeader: ({
    title,
    subtitle,
  }: {
    title: string;
    subtitle: string;
    metadata?: React.ReactNode;
    actions?: React.ReactNode;
    tabs?: React.ReactNode;
    className?: string;
  }) => (
    <div data-testid="view-header">
      <h1>{title}</h1>
      <p>{subtitle}</p>
    </div>
  ),
}));

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
  }: {
    children: React.ReactNode;
    href: string;
  }) => <a href={href}>{children}</a>,
}));

vi.mock("sonner", () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
}));

import Dashboard from "@/app/page";

// Tests the Dashboard page: heading, subtitle, status section, idle state,
// infrastructure section, insight cards, primes table, service cards, agent
// controller, fleet stats, database metrics, action buttons, charts, and feed.
describe("Dashboard", () => {
  it("renders dashboard heading", () => {
    render(<Dashboard />);
    expect(screen.getByText("Dashboard")).toBeInTheDocument();
  });

  it("renders dashboard subtitle", () => {
    render(<Dashboard />);
    expect(
      screen.getByText(
        "Real-time prime search monitoring, fleet health, and discovery history"
      )
    ).toBeInTheDocument();
  });

  it("renders current status section", () => {
    render(<Dashboard />);
    expect(screen.getByText("Current status")).toBeInTheDocument();
  });

  it("shows Idle when no active search", () => {
    render(<Dashboard />);
    expect(screen.getByText("Idle")).toBeInTheDocument();
  });

  it("renders infrastructure section", () => {
    render(<Dashboard />);
    expect(screen.getByText("Infrastructure")).toBeInTheDocument();
  });

  it("renders insights section", () => {
    render(<Dashboard />);
    expect(screen.getByText("Insights")).toBeInTheDocument();
  });

  it("renders primes table", () => {
    render(<Dashboard />);
    expect(screen.getByTestId("primes-table")).toBeInTheDocument();
  });

  it("renders service status cards", () => {
    render(<Dashboard />);
    expect(screen.getByTestId("service-coordinator")).toBeInTheDocument();
    expect(screen.getByTestId("service-database")).toBeInTheDocument();
  });

  it("renders agent controller card", () => {
    render(<Dashboard />);
    expect(screen.getByTestId("agent-controller")).toBeInTheDocument();
  });

  it("renders fleet stats strip", () => {
    render(<Dashboard />);
    expect(screen.getByText("Servers")).toBeInTheDocument();
    expect(screen.getByText("Cores")).toBeInTheDocument();
    expect(screen.getByText("Active Searches")).toBeInTheDocument();
    expect(screen.getByText("Candidates Tested")).toBeInTheDocument();
  });

  it("renders database service with stored primes count", () => {
    render(<Dashboard />);
    expect(screen.getByText("42 primes stored")).toBeInTheDocument();
  });

  it("renders form count in database service", () => {
    render(<Dashboard />);
    expect(screen.getByText("2 forms indexed")).toBeInTheDocument();
  });

  it("renders New Search button", () => {
    render(<Dashboard />);
    // There are multiple "New Search" buttons (in header actions and infra section)
    const buttons = screen.getAllByText("New Search");
    expect(buttons.length).toBeGreaterThanOrEqual(1);
  });

  it("renders Add Server button", () => {
    render(<Dashboard />);
    expect(screen.getByText("Add Server")).toBeInTheDocument();
  });

  it("renders insight cards", () => {
    render(<Dashboard />);
    expect(screen.getByTestId("insight-cards")).toBeInTheDocument();
  });

  it("renders form leaderboard", () => {
    render(<Dashboard />);
    expect(screen.getByTestId("form-leaderboard")).toBeInTheDocument();
  });

  it("renders charts", () => {
    render(<Dashboard />);
    expect(screen.getByTestId("discovery-timeline")).toBeInTheDocument();
    expect(screen.getByTestId("digit-distribution")).toBeInTheDocument();
  });

  it("renders activity feed", () => {
    render(<Dashboard />);
    expect(screen.getByTestId("activity-feed")).toBeInTheDocument();
  });
});
