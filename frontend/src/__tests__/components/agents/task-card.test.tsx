/**
 * @file Tests for the agent TaskCard component
 * @module __tests__/components/agents/task-card
 *
 * Validates the task card that displays an AI agent task on the Agents page
 * Tasks tab. Each card shows the task title, status badge, priority badge,
 * action buttons (Details, Cancel), subtask progress tree, and cost info.
 * Tests cover title rendering, status badge, Details/Cancel buttons, Cancel
 * button visibility for completed tasks, subtask tree progress display, and
 * cost/budget formatting.
 *
 * @see {@link ../../../components/agents/task-card} Source component
 * @see {@link ../../../hooks/use-agents} useAgentTasks, cancelTask
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/hooks/use-agents", () => ({
  useAgentEvents: () => ({
    events: [],
    loading: false,
  }),
  useAgentTimeline: () => ({
    events: [],
    loading: false,
  }),
  useAgentLogs: () => ({
    logs: [],
    total: 0,
    loading: false,
    offset: 0,
    setOffset: vi.fn(),
    limit: 100,
  }),
  cancelTask: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@/components/agents/helpers", () => ({
  statusBadge: (status: string) => <span data-testid="status-badge">{status}</span>,
  priorityBadge: (priority: string) => <span data-testid="priority-badge">{priority}</span>,
  eventIcon: (type: string) => <span data-testid="event-icon">{type}</span>,
  statusDot: (status: string) => <span data-testid="status-dot">{status}</span>,
  roleBadge: (role: string | null) =>
    role ? <span data-testid="role-badge">{role}</span> : null,
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

vi.mock("@/lib/format", () => ({
  numberWithCommas: (x: number) => String(x),
  formatTime: (t: string) => t,
  relativeTime: (t: string) => t,
}));

import { TaskCard } from "@/components/agents/task-card";

const baseTask = {
  id: 1,
  title: "Optimize sieve module",
  status: "in_progress",
  priority: "high",
  role_name: "engine",
  template_name: null,
  agent_model: "opus",
  permission_level: 1,
  created_at: "2026-01-01T00:00:00Z",
  completed_at: null,
  tokens_used: 15000,
  cost_usd: 0.45,
  max_cost_usd: 5.0,
  parent_id: null,
};

// Tests the TaskCard: title, status badge, Details/Cancel buttons,
// completed state (no Cancel), subtask tree, and cost/budget display.
describe("TaskCard", () => {
  it("renders task title", () => {
    render(<TaskCard task={baseTask as any} children={[]} />);
    expect(screen.getByText("Optimize sieve module")).toBeInTheDocument();
  });

  it("shows status badge", () => {
    render(<TaskCard task={baseTask as any} children={[]} />);
    expect(screen.getByTestId("status-badge")).toHaveTextContent("in_progress");
  });

  it("shows Details button", () => {
    render(<TaskCard task={baseTask as any} children={[]} />);
    expect(screen.getByText("Details")).toBeInTheDocument();
  });

  it("shows Cancel button for active tasks", () => {
    render(<TaskCard task={baseTask as any} children={[]} />);
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("hides Cancel button for completed tasks", () => {
    const completedTask = { ...baseTask, status: "completed" };
    render(<TaskCard task={completedTask as any} children={[]} />);
    expect(screen.queryByText("Cancel")).not.toBeInTheDocument();
  });

  it("renders subtask tree when children provided", () => {
    const children = [
      {
        id: 2,
        title: "Step 1: Profile",
        status: "completed",
        priority: "normal",
        role_name: null,
        template_name: null,
        agent_model: null,
        permission_level: 1,
        created_at: "2026-01-01T00:00:00Z",
        completed_at: "2026-01-01T01:00:00Z",
        tokens_used: 0,
        cost_usd: 0,
        max_cost_usd: null,
        parent_id: 1,
      },
      {
        id: 3,
        title: "Step 2: Implement",
        status: "in_progress",
        priority: "normal",
        role_name: null,
        template_name: null,
        agent_model: null,
        permission_level: 1,
        created_at: "2026-01-01T00:00:00Z",
        completed_at: null,
        tokens_used: 0,
        cost_usd: 0,
        max_cost_usd: null,
        parent_id: 1,
      },
    ];
    render(<TaskCard task={baseTask as any} children={children as any} />);
    expect(screen.getByText("1/2 steps complete")).toBeInTheDocument();
  });

  it("displays cost information", () => {
    render(<TaskCard task={baseTask as any} children={[]} />);
    expect(screen.getByText("$0.4500")).toBeInTheDocument();
    expect(screen.getByText("max $5.00")).toBeInTheDocument();
  });
});
