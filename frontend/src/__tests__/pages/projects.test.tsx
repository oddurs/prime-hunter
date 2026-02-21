/**
 * @file Tests for the Projects page
 * @module __tests__/pages/projects
 *
 * Validates the Projects page at `/projects`, which provides campaign-style
 * prime discovery management. Each project targets a specific prime form
 * with configurable objectives, budgets, and phases. The page consumes
 * WebSocket context for project and world record data. Tests verify page
 * heading, subtitle, "New Project" button, status filter tabs (Active,
 * Draft, Paused, Completed, All), and empty state rendering.
 *
 * @see {@link ../../app/projects/page} Source page
 * @see {@link ../../hooks/use-projects} Project data hooks
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("next/navigation", () => ({
  useSearchParams: () => new URLSearchParams(),
}));

vi.mock("@/contexts/websocket-context", () => ({
  useWs: () => ({
    projects: [],
    records: [],
    connected: true,
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

vi.mock("@/components/empty-state", () => ({
  EmptyState: ({ message }: { message: string }) => (
    <div data-testid="empty-state">{message}</div>
  ),
}));

vi.mock("@/components/new-project-dialog", () => ({
  NewProjectDialog: () => null,
}));

vi.mock("@/components/project-card", () => ({
  ProjectCard: () => <div data-testid="project-card">Project</div>,
}));

vi.mock("@/components/record-comparison", () => ({
  RecordComparison: () => <div data-testid="record-comparison">Record</div>,
}));

vi.mock("@/components/phase-timeline", () => ({
  PhaseTimeline: () => <div data-testid="phase-timeline">Timeline</div>,
}));

vi.mock("@/components/cost-tracker", () => ({
  CostTracker: () => <div data-testid="cost-tracker">Cost</div>,
}));

vi.mock("@/components/charts/cost-history", () => ({
  CostHistoryChart: () => <div data-testid="cost-history">Cost History</div>,
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
  toast: { success: vi.fn(), error: vi.fn() },
}));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) => String(x),
  formatTime: (t: string) => t,
}));

import ProjectsPage from "@/app/projects/page";

// Tests the ProjectsPage: heading, subtitle, New Project button, status
// filter tabs, and empty state rendering.
describe("ProjectsPage", () => {
  it("renders without crashing", () => {
    render(<ProjectsPage />);
    expect(screen.getByText("Projects")).toBeInTheDocument();
  });

  it("shows project subtitle", () => {
    render(<ProjectsPage />);
    expect(
      screen.getByText("Campaign-style prime discovery management")
    ).toBeInTheDocument();
  });

  it("shows New Project button", () => {
    render(<ProjectsPage />);
    expect(screen.getByText("New Project")).toBeInTheDocument();
  });

  it("renders tabs for Active, Draft, Paused, Completed, All", () => {
    render(<ProjectsPage />);
    expect(screen.getByText("Active")).toBeInTheDocument();
    expect(screen.getByText("Draft")).toBeInTheDocument();
    expect(screen.getByText("Paused")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
    expect(screen.getByText(/All/)).toBeInTheDocument();
  });

  it("shows empty state when no projects", () => {
    render(<ProjectsPage />);
    const emptyStates = screen.getAllByTestId("empty-state");
    expect(emptyStates.length).toBeGreaterThanOrEqual(1);
  });
});
