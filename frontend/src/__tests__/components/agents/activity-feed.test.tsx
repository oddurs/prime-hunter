/**
 * @file Tests for the agent ActivityFeed component
 * @module __tests__/components/agents/activity-feed
 *
 * Validates the agent activity feed that displays a chronological stream of
 * agent events (task started, completed, etc.) on the Agents page Activity tab.
 * Tests cover loading state, empty state, event summary rendering, task ID
 * references, agent name display, and event type icons.
 *
 * @see {@link ../../../components/agents/activity-feed} Source component
 * @see {@link ../../../hooks/use-agents} useAgentEvents hook
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

const mockUseAgentEvents = vi.fn();

vi.mock("@/hooks/use-agents", () => ({
  useAgentEvents: (...args: unknown[]) => mockUseAgentEvents(...args),
}));

vi.mock("@/components/empty-state", () => ({
  EmptyState: ({ message }: { message: string }) => (
    <div data-testid="empty-state">{message}</div>
  ),
}));

vi.mock("@/components/agents/helpers", () => ({
  eventIcon: (type: string) => <span data-testid="event-icon">{type}</span>,
}));

vi.mock("@/lib/format", () => ({
  formatTime: (t: string) => t,
}));

import { ActivityFeed } from "@/components/agents/activity-feed";

// Tests the agent ActivityFeed: loading state, empty state, event summaries,
// task ID references, agent name badges, and event type icons.
describe("ActivityFeed", () => {
  /** Verifies the "Loading activity..." message while events are being fetched. */
  it("shows loading state", () => {
    mockUseAgentEvents.mockReturnValue({
      events: [],
      loading: true,
    });
    render(<ActivityFeed />);
    expect(screen.getByText("Loading activity...")).toBeInTheDocument();
  });

  /** Verifies the empty state message when no agent events have been recorded. */
  it("shows empty state when no events", () => {
    mockUseAgentEvents.mockReturnValue({
      events: [],
      loading: false,
    });
    render(<ActivityFeed />);
    expect(screen.getByText("No agent activity yet.")).toBeInTheDocument();
  });

  /** Verifies each event's summary text renders in the feed. */
  it("renders event summaries", () => {
    mockUseAgentEvents.mockReturnValue({
      events: [
        {
          id: 1,
          event_type: "started",
          summary: "Task started: Engine optimization",
          task_id: 42,
          agent: "opus-agent",
          created_at: "2026-01-01T00:00:00Z",
        },
        {
          id: 2,
          event_type: "completed",
          summary: "Task completed: Build fix",
          task_id: 43,
          agent: null,
          created_at: "2026-01-01T01:00:00Z",
        },
      ],
      loading: false,
    });
    render(<ActivityFeed />);
    expect(screen.getByText("Task started: Engine optimization")).toBeInTheDocument();
    expect(screen.getByText("Task completed: Build fix")).toBeInTheDocument();
  });

  /** Verifies the "task #N" reference appears when an event has a task_id. */
  it("shows task IDs when present", () => {
    mockUseAgentEvents.mockReturnValue({
      events: [
        {
          id: 1,
          event_type: "started",
          summary: "Search started",
          task_id: 42,
          agent: null,
          created_at: "2026-01-01T00:00:00Z",
        },
      ],
      loading: false,
    });
    render(<ActivityFeed />);
    expect(screen.getByText("task #42")).toBeInTheDocument();
  });

  /** Verifies the agent name badge renders when the event has an agent field. */
  it("shows agent name when present", () => {
    mockUseAgentEvents.mockReturnValue({
      events: [
        {
          id: 1,
          event_type: "started",
          summary: "Started",
          task_id: null,
          agent: "opus-agent",
          created_at: "2026-01-01T00:00:00Z",
        },
      ],
      loading: false,
    });
    render(<ActivityFeed />);
    expect(screen.getByText("opus-agent")).toBeInTheDocument();
  });

  /** Verifies each event renders its type-specific icon via the eventIcon helper. */
  it("renders event icons", () => {
    mockUseAgentEvents.mockReturnValue({
      events: [
        {
          id: 1,
          event_type: "started",
          summary: "Test",
          task_id: null,
          agent: null,
          created_at: "2026-01-01T00:00:00Z",
        },
      ],
      loading: false,
    });
    render(<ActivityFeed />);
    expect(screen.getByTestId("event-icon")).toBeInTheDocument();
  });
});
