/**
 * @file Tests for the use-agents hook (aggregate agent data hook)
 * @module __tests__/hooks/use-agents
 *
 * Tests the consolidated agent hooks exported from use-agents.ts, which
 * provides a unified API for agent task listing, role management, and
 * task tree construction. This is the primary hook consumed by the
 * agents page component, combining data from agent_tasks and agent_roles
 * Supabase tables with Realtime subscriptions.
 *
 * The mock chain handles two query patterns:
 * - With limit: from().select().order().limit() -> resolves with data
 * - Without limit: from().select().order() -> resolves via thenable
 *
 * @see {@link ../../hooks/use-agents} Source hook
 * @see {@link ../../__mocks__/supabase} Supabase mock configuration
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock supabase before importing the hook
const mockSelect = vi.fn();
const mockEq = vi.fn();
const mockOrder = vi.fn();
const mockLimit = vi.fn();
const mockFrom = vi.fn();
const mockChannel = vi.fn();
const mockOn = vi.fn();
const mockSubscribe = vi.fn();
const mockRemoveChannel = vi.fn();
const mockInsert = vi.fn();
const mockUpdate = vi.fn();
const mockIn = vi.fn();
const mockSingle = vi.fn();

/**
 * Configures the mock query chain supporting both limited and unlimited queries.
 * Limited queries (tasks): chain ends with .limit() resolving directly.
 * Unlimited queries (roles): chain ends with .order() which is thenable.
 */
function setupChain(finalData: unknown, finalError: unknown) {
  const chain = {
    select: mockSelect.mockReturnThis(),
    eq: mockEq.mockReturnThis(),
    order: mockOrder.mockReturnThis(),
    limit: mockLimit.mockResolvedValue({
      data: finalData,
      error: finalError,
    }),
    insert: mockInsert.mockReturnThis(),
    update: mockUpdate.mockReturnThis(),
    in: mockIn.mockReturnThis(),
    single: mockSingle.mockResolvedValue({
      data: finalData,
      error: finalError,
    }),
  };
  // For queries without limit (like templates and roles), order resolves directly
  mockOrder.mockReturnValue({
    ...chain,
    then: vi.fn((resolve: (v: unknown) => void) =>
      resolve({ data: finalData, error: finalError })
    ),
  });
  mockFrom.mockReturnValue(chain);
  return chain;
}

// Realtime channel mock for live task and role updates
const channelMock = {
  on: mockOn.mockReturnThis(),
  subscribe: mockSubscribe.mockReturnThis(),
};
mockChannel.mockReturnValue(channelMock);

vi.mock("@/lib/supabase", () => ({
  supabase: {
    from: (...args: unknown[]) => mockFrom(...args),
    channel: (...args: unknown[]) => mockChannel(...args),
    removeChannel: (...args: unknown[]) => mockRemoveChannel(...args),
  },
}));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
}));

import { useAgentTasks, useAgentRoles, buildTaskTree } from "@/hooks/use-agents";
import type { AgentTask } from "@/hooks/use-agents";

// Tests the consolidated agent task listing hook with Realtime subscription.
describe("useAgentTasks", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies that the hook fetches tasks on mount with full field projection.
   * The mock data represents a completed factorial analysis task with
   * Claude Opus model, 5000 tokens used, and $0.15 cost.
   */
  it("fetches agent tasks on mount", async () => {
    const mockData = [
      {
        id: 1,
        title: "Analyze factorial search",
        description: "Run analysis on factorial search results",
        status: "completed",
        priority: "high",
        agent_model: "claude-opus-4-20250514",
        assigned_agent: "agent-1",
        source: "manual",
        result: null,
        tokens_used: 5000,
        cost_usd: 0.15,
        created_at: "2026-01-01T00:00:00Z",
        started_at: "2026-01-01T00:01:00Z",
        completed_at: "2026-01-01T00:05:00Z",
        parent_task_id: null,
        max_cost_usd: 1.0,
        permission_level: 1,
        template_name: null,
        on_child_failure: "continue",
        role_name: null,
      },
    ];
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(result.current.tasks).toHaveLength(1);
    });
    expect(result.current.tasks[0].title).toBe("Analyze factorial search");
    expect(result.current.loading).toBe(false);
    expect(mockFrom).toHaveBeenCalledWith("agent_tasks");
  });

  /** Verifies graceful error handling; tasks defaults to empty array. */
  it("returns empty on error", async () => {
    setupChain(null, { message: "Permission denied" });

    const { result } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.tasks).toEqual([]);
  });

  /**
   * Verifies Realtime subscription to all events on agent_tasks table
   * for live task status updates.
   */
  it("subscribes to realtime changes", async () => {
    setupChain([], null);

    renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalledWith("agent_tasks_changes");
    });
    expect(mockOn).toHaveBeenCalledWith(
      "postgres_changes",
      { event: "*", schema: "public", table: "agent_tasks" },
      expect.any(Function)
    );
  });
});

// Tests the agent role listing hook from the consolidated use-agents module.
describe("useAgentRoles", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies role listing from the agent_roles table. Each role defines
   * domain restrictions, default model, permission level, and cost limits.
   */
  it("fetches roles on mount", async () => {
    const mockData = [
      {
        id: 1,
        name: "engine-analyst",
        description: "Analyzes prime search engine performance",
        domains: ["engine"],
        default_permission_level: 1,
        default_model: "claude-opus-4-20250514",
        system_prompt: null,
        default_max_cost_usd: 5.0,
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      },
    ];
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentRoles());

    await waitFor(() => {
      expect(result.current.roles).toHaveLength(1);
    });
    expect(result.current.roles[0].name).toBe("engine-analyst");
    expect(result.current.loading).toBe(false);
    expect(mockFrom).toHaveBeenCalledWith("agent_roles");
  });

  /** Verifies graceful error handling when the roles table is missing. */
  it("returns empty on error", async () => {
    setupChain(null, { message: "Table missing" });

    const { result } = renderHook(() => useAgentRoles());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.roles).toEqual([]);
  });
});

// Tests the buildTaskTree pure function from the consolidated use-agents module.
// Transforms flat task list with parent_task_id references into a tree structure.
describe("buildTaskTree", () => {
  /**
   * Verifies hierarchical tree construction from a flat task list.
   * Parent (id=1) gets two children, Standalone (id=4) becomes a separate root.
   */
  it("builds tree from flat task list", () => {
    const tasks: AgentTask[] = [
      makeTask({ id: 1, title: "Parent", parent_task_id: null }),
      makeTask({ id: 2, title: "Child 1", parent_task_id: 1 }),
      makeTask({ id: 3, title: "Child 2", parent_task_id: 1 }),
      makeTask({ id: 4, title: "Standalone", parent_task_id: null }),
    ];

    const tree = buildTaskTree(tasks);

    expect(tree).toHaveLength(2);
    expect(tree[0].task.title).toBe("Parent");
    expect(tree[0].children).toHaveLength(2);
    expect(tree[0].children[0].title).toBe("Child 1");
    expect(tree[0].children[1].title).toBe("Child 2");
    expect(tree[1].task.title).toBe("Standalone");
    expect(tree[1].children).toHaveLength(0);
  });

  /** Verifies empty input produces empty tree. */
  it("returns empty array for empty input", () => {
    const tree = buildTaskTree([]);
    expect(tree).toEqual([]);
  });

  /** Verifies children are sorted by id ascending regardless of input order. */
  it("sorts children by id", () => {
    const tasks: AgentTask[] = [
      makeTask({ id: 1, title: "Parent", parent_task_id: null }),
      makeTask({ id: 5, title: "Child B", parent_task_id: 1 }),
      makeTask({ id: 3, title: "Child A", parent_task_id: 1 }),
    ];

    const tree = buildTaskTree(tasks);

    expect(tree[0].children[0].id).toBe(3);
    expect(tree[0].children[1].id).toBe(5);
  });
});

/**
 * Factory function for creating test AgentTask objects with sensible defaults.
 * All optional fields default to null/0, status defaults to "pending".
 */
function makeTask(overrides: Partial<AgentTask> = {}): AgentTask {
  return {
    id: 1,
    title: "Test task",
    description: "Test description",
    status: "pending",
    priority: "medium",
    agent_model: null,
    assigned_agent: null,
    source: "manual",
    result: null,
    tokens_used: 0,
    cost_usd: 0,
    created_at: "2026-01-01T00:00:00Z",
    started_at: null,
    completed_at: null,
    parent_task_id: null,
    max_cost_usd: null,
    permission_level: 1,
    template_name: null,
    on_child_failure: "continue",
    role_name: null,
    ...overrides,
  };
}
