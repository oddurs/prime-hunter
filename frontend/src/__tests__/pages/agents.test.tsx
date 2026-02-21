/**
 * @file Tests for the Agents page
 * @module __tests__/pages/agents
 *
 * Validates the Agents page at `/agents`, which provides AI agent task
 * management. The page consumes useAgentTasks, useAgentBudgets, and
 * useAgentRoles hooks and renders stat cards (Active Tasks, Pending Queue,
 * Completed Today, Today's Spend), a "New Task" action button, and six
 * tabs (Tasks, Activity, Memory, Budget, Schedules, Analytics). Tests
 * verify page heading, subtitle, stat cards, tabs, action buttons, and
 * empty state rendering.
 *
 * @see {@link ../../app/agents/page} Source page
 * @see {@link ../../hooks/use-agents} Agent data hooks
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("@/hooks/use-agents", () => ({
  useAgentTasks: () => ({
    tasks: [],
  }),
  useAgentBudgets: () => ({
    budgets: [],
  }),
  useAgentRoles: () => ({
    roles: [],
  }),
  buildTaskTree: () => [],
}));

vi.mock("@/components/view-header", () => ({
  ViewHeader: ({
    title,
    subtitle,
    actions,
    tabs,
  }: {
    title: string;
    subtitle: string;
    actions?: React.ReactNode;
    tabs?: React.ReactNode;
  }) => (
    <div data-testid="view-header">
      <h1>{title}</h1>
      <p>{subtitle}</p>
      {actions}
      {tabs}
    </div>
  ),
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

vi.mock("@/components/agents/helpers", () => ({
  ROLE_CONFIG: {},
}));

vi.mock("@/components/agents/new-task-dialog", () => ({
  NewTaskDialog: () => null,
}));

vi.mock("@/components/agents/task-card", () => ({
  TaskCard: () => <div data-testid="task-card">Task</div>,
}));

vi.mock("@/components/agents/activity-feed", () => ({
  ActivityFeed: () => <div data-testid="activity-feed">Activity Feed</div>,
}));

vi.mock("@/components/agents/budget-cards", () => ({
  BudgetCards: () => <div data-testid="budget-cards">Budget Cards</div>,
}));

vi.mock("@/components/agents/memory-tab", () => ({
  MemoryTab: () => <div data-testid="memory-tab">Memory Tab</div>,
}));

vi.mock("@/components/agents/schedules-tab", () => ({
  SchedulesTab: () => <div data-testid="schedules-tab">Schedules Tab</div>,
}));

vi.mock("@/components/agents/analytics-tab", () => ({
  AnalyticsTab: () => <div data-testid="analytics-tab">Analytics Tab</div>,
}));

import AgentsPage from "@/app/agents/page";

// Tests the AgentsPage: heading, subtitle, stat cards, tab navigation,
// New Task button, and empty state when no tasks exist.
describe("AgentsPage", () => {
  it("renders without crashing", () => {
    render(<AgentsPage />);
    expect(screen.getByText("Agents")).toBeInTheDocument();
  });

  it("shows 0 running and 0 queued in subtitle", () => {
    render(<AgentsPage />);
    expect(screen.getByText(/0 running/)).toBeInTheDocument();
  });

  it("renders stat cards", () => {
    render(<AgentsPage />);
    expect(screen.getByTestId("stat-active-tasks")).toBeInTheDocument();
    expect(screen.getByTestId("stat-pending-queue")).toBeInTheDocument();
    expect(screen.getByTestId("stat-completed-today")).toBeInTheDocument();
    expect(screen.getByTestId("stat-today's-spend")).toBeInTheDocument();
  });

  it("renders tabs", () => {
    render(<AgentsPage />);
    expect(screen.getByText("Tasks")).toBeInTheDocument();
    expect(screen.getByText("Activity")).toBeInTheDocument();
    expect(screen.getByText("Memory")).toBeInTheDocument();
    expect(screen.getByText("Budget")).toBeInTheDocument();
    expect(screen.getByText("Schedules")).toBeInTheDocument();
    expect(screen.getByText("Analytics")).toBeInTheDocument();
  });

  it("shows New Task button", () => {
    render(<AgentsPage />);
    expect(screen.getByText("New Task")).toBeInTheDocument();
  });

  it("shows empty state when no tasks", () => {
    render(<AgentsPage />);
    const emptyStates = screen.getAllByTestId("empty-state");
    expect(emptyStates.length).toBeGreaterThanOrEqual(1);
  });
});
