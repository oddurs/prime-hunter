/**
 * @file Tests for the Network (Fleet) page
 * @module __tests__/pages/network
 *
 * Validates the Network page at `/network`, which provides fleet management
 * with server roles, worker health monitoring, deployment lifecycle, and
 * distributed search operations. The page consumes WebSocket context for
 * real-time fleet/coordinator/deployment data. Tests verify page heading,
 * subtitle, stat cards (Servers, Workers, Cores), action buttons (Add Server,
 * New Search), and empty states for compute servers and deployments.
 *
 * @see {@link ../../app/network/page} Source page
 * @see {@link ../../contexts/websocket-context} WebSocket data provider
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("@/contexts/websocket-context", () => ({
  useWs: () => ({
    fleet: {
      workers: [],
      servers: [],
      total_workers: 0,
      total_cores: 0,
      total_tested: 0,
      total_found: 0,
    },
    coordinator: null,
    deployments: [],
    searches: [],
    connected: true,
  }),
}));

vi.mock("@/components/view-header", () => ({
  ViewHeader: ({
    title,
    subtitle,
    actions,
  }: {
    title: string;
    subtitle: string;
    actions?: React.ReactNode;
  }) => (
    <div data-testid="view-header">
      <h1>{title}</h1>
      <p>{subtitle}</p>
      {actions}
    </div>
  ),
}));

vi.mock("@/components/add-server-dialog", () => ({
  AddServerDialog: () => null,
}));

vi.mock("@/components/new-search-dialog", () => ({
  NewSearchDialog: () => null,
}));

vi.mock("@/components/search-card", () => ({
  SearchCard: () => <div data-testid="search-card">Search</div>,
}));

vi.mock("@/components/metrics-bar", () => ({
  MetricsBar: () => <div data-testid="metrics-bar">MetricsBar</div>,
}));

vi.mock("@/components/worker-detail-dialog", () => ({
  WorkerDetailDialog: () => null,
}));

vi.mock("@/components/stat-card", () => ({
  StatCard: ({ label, value }: { label: string; value: React.ReactNode }) => (
    <div data-testid={`stat-${label.toLowerCase().replace(/\s+/g, "-")}`}>
      {label}: {value}
    </div>
  ),
}));

vi.mock("@/components/empty-state", () => ({
  EmptyState: ({ message }: { message: string }) => (
    <div data-testid="empty-state">{message}</div>
  ),
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  formatTime: (t: string) => t,
  numberWithCommas: (x: number) => String(x),
}));

// FleetPage is actually the network page at src/app/network/page.tsx
import FleetPage from "@/app/network/page";

// Tests the FleetPage (Network): heading, subtitle, stat cards, action buttons,
// and empty states for compute servers and deployments.
describe("FleetPage (Network)", () => {
  it("renders without crashing", () => {
    render(<FleetPage />);
    expect(screen.getByText("Fleet")).toBeInTheDocument();
  });

  it("shows fleet subtitle", () => {
    render(<FleetPage />);
    expect(
      screen.getByText(
        "Server roles, worker health, deployment lifecycle, and distributed search operations."
      )
    ).toBeInTheDocument();
  });

  it("renders stat cards", () => {
    render(<FleetPage />);
    expect(screen.getByTestId("stat-servers")).toBeInTheDocument();
    expect(screen.getByTestId("stat-workers")).toBeInTheDocument();
    expect(screen.getByTestId("stat-cores")).toBeInTheDocument();
  });

  it("shows Add Server and New Search buttons", () => {
    render(<FleetPage />);
    expect(screen.getByText("Add Server")).toBeInTheDocument();
    expect(screen.getByText("New Search")).toBeInTheDocument();
  });

  it("shows empty state for compute servers", () => {
    render(<FleetPage />);
    expect(screen.getByText("No compute servers online.")).toBeInTheDocument();
  });

  it("shows empty state for deployments", () => {
    render(<FleetPage />);
    expect(screen.getByText("No deployments yet.")).toBeInTheDocument();
  });
});
