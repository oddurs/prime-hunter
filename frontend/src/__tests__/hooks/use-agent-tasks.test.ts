/**
 * @file Tests for useAgentTasks hook and related agent infrastructure hooks/functions
 * @module __tests__/hooks/use-agent-tasks
 *
 * Comprehensive test suite for the agent task management system. Covers:
 * - useAgentTasks: Task listing with status filtering, Realtime subscriptions
 * - useAgentEvents: Event stream for agent activity (tool calls, completions)
 * - useAgentTemplates: Multi-step task template definitions
 * - useAgentRoles: Role-based agent configurations (domains, permissions, models)
 * - useAgentLogs: Task execution log retrieval via REST API
 * - useAgentTimeline: Task timeline events via REST API
 * - createTask: Supabase INSERT for new agent tasks
 * - cancelTask: Status update with guard against already-completed tasks
 * - expandTemplate: REST API template expansion into parent+child task tree
 * - buildTaskTree: Pure function to transform flat task list into hierarchical tree
 *
 * The mock chain uses a "thenable" pattern because the hook builds queries
 * dynamically (query = query.limit(200); query = query.eq(...)) requiring
 * each chain link to support both further chaining and await resolution.
 *
 * @see {@link ../../hooks/use-agent-tasks} Source hooks and functions
 * @see {@link ../../__mocks__/supabase} Supabase mock configuration
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// --- Supabase mock ---
// Mocks the full Supabase query chain with thenable support for dynamic
// query building. Each method returns an object that is both chainable
// and thenable (can be awaited or further chained).
const mockSelect = vi.fn();
const mockEq = vi.fn();
const mockOrder = vi.fn();
const mockLimit = vi.fn();
const mockFrom = vi.fn();
const mockInsert = vi.fn();
const mockUpdate = vi.fn();
const mockIn = vi.fn();
const mockSingle = vi.fn();
const mockChannel = vi.fn();
const mockOn = vi.fn();
const mockSubscribe = vi.fn();
const mockRemoveChannel = vi.fn();

/**
 * Creates a thenable chain mock that supports dynamic query building.
 * The hook pattern `query = query.limit(200); query = query.eq(...)` requires
 * each link in the chain to return an object with both chaining methods
 * and a `.then()` for async resolution.
 */
function setupChain(finalData: unknown, finalError: unknown) {
  const resolveResult = { data: finalData, error: finalError };

  // Create a thenable object that also supports further chaining (.eq, etc.)
  // This is needed because useAgentTasks does: query = query.limit(200); query = query.eq(...);
  function makeThenableChain(): Record<string, unknown> {
    const obj: Record<string, unknown> = {};
    obj.eq = mockEq.mockImplementation(() => makeThenableChain());
    obj.select = mockSelect.mockReturnValue(obj);
    obj.order = mockOrder.mockReturnValue(obj);
    obj.limit = mockLimit.mockReturnValue(obj);
    obj.in = mockIn.mockImplementation(() => Promise.resolve({ error: finalError }));
    obj.single = mockSingle.mockImplementation(() => Promise.resolve(resolveResult));
    obj.then = (resolve: (v: unknown) => void) => resolve(resolveResult);
    return obj;
  }

  const chain = makeThenableChain();

  // Also support insert/update chains
  chain.insert = mockInsert.mockReturnValue(chain);
  chain.update = mockUpdate.mockReturnValue(chain);

  mockFrom.mockReturnValue(chain);
  return chain;
}

// Realtime channel mock for live task/event/template change notifications
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

// --- fetch mock for REST API hooks ---
// useAgentLogs, useAgentTimeline, and expandTemplate use the REST API via fetch.
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
}));

import {
  useAgentTasks,
  useAgentEvents,
  useAgentTemplates,
  useAgentRoles,
  useAgentLogs,
  useAgentTimeline,
  createTask,
  cancelTask,
  expandTemplate,
  buildTaskTree,
} from "@/hooks/use-agent-tasks";
import type { AgentTask } from "@/hooks/use-agent-tasks";

/**
 * Factory function to create a complete AgentTask object with sensible defaults.
 * Produces a pending, manually-created task with no cost or token usage.
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

// Tests the task listing hook: mount -> loading -> data -> filtering -> Realtime.
// Validates Supabase query construction with optional status filter, ordering,
// limit, and Realtime subscription lifecycle.
describe("useAgentTasks", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies that the hook fetches agent tasks on mount from the agent_tasks
   * table and exposes them through the returned tasks array.
   */
  it("fetches tasks on mount", async () => {
    const mockData = [makeTask({ title: "Analyze search" })];
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(result.current.tasks).toHaveLength(1);
    });
    expect(result.current.tasks[0].title).toBe("Analyze search");
    expect(result.current.loading).toBe(false);
    expect(mockFrom).toHaveBeenCalledWith("agent_tasks");
  });

  /** Verifies graceful error handling; tasks defaults to empty array. */
  it("returns empty array on error", async () => {
    setupChain(null, { message: "Permission denied" });

    const { result } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.tasks).toEqual([]);
  });

  /**
   * Verifies that passing a status string (e.g., "in_progress") to the hook
   * appends an .eq("status", value) filter to the Supabase query, narrowing
   * results to only tasks with that status.
   */
  it("applies status filter when provided", async () => {
    setupChain([], null);

    renderHook(() => useAgentTasks("in_progress"));

    await waitFor(() => {
      expect(mockEq).toHaveBeenCalledWith("status", "in_progress");
    });
  });

  /**
   * Verifies that omitting the status parameter does NOT add a status filter.
   * The .eq() mock should not be called with "status" as the first argument.
   */
  it("does not apply status filter when undefined", async () => {
    setupChain([], null);

    renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(mockFrom).toHaveBeenCalledWith("agent_tasks");
    });
    // eq should not be called for status (though it might be called during limit chain setup)
    expect(mockEq).not.toHaveBeenCalledWith("status", expect.any(String));
  });

  /**
   * Verifies Realtime subscription to all events (*) on agent_tasks table
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

  /** Verifies Realtime channel cleanup on unmount. */
  it("cleans up subscription on unmount", async () => {
    setupChain([], null);

    const { unmount } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalled();
    });

    unmount();
    expect(mockRemoveChannel).toHaveBeenCalled();
  });

  /**
   * Verifies that tasks are ordered by created_at descending (newest first)
   * and limited to 200 results to avoid loading excessive data.
   */
  it("orders tasks by created_at descending with limit 200", async () => {
    setupChain([], null);

    renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(mockOrder).toHaveBeenCalledWith("created_at", { ascending: false });
      expect(mockLimit).toHaveBeenCalledWith(200);
    });
  });
});

// Tests the agent events hook which provides an activity stream of agent actions
// (tool calls, completions, errors). Supports optional filtering by task ID.
describe("useAgentEvents", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies that the hook fetches all events when no taskId filter is provided.
   * Events include agent lifecycle events (started, completed) and tool calls.
   */
  it("fetches events on mount without taskId filter", async () => {
    const mockData = [
      { id: 1, task_id: null, event_type: "agent_started", agent: "agent-1", summary: "Agent started", detail: null, created_at: "2026-01-01", tool_name: null, input_tokens: null, output_tokens: null, duration_ms: null },
    ];
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentEvents());

    await waitFor(() => {
      expect(result.current.events).toHaveLength(1);
    });
    expect(result.current.events[0].event_type).toBe("agent_started");
    expect(mockFrom).toHaveBeenCalledWith("agent_events");
  });

  /**
   * Verifies that passing a taskId filters events to only those belonging
   * to the specified task, using .eq("task_id", taskId).
   */
  it("filters events by taskId when provided", async () => {
    setupChain([], null);

    renderHook(() => useAgentEvents(42));

    await waitFor(() => {
      expect(mockEq).toHaveBeenCalledWith("task_id", 42);
    });
  });

  /**
   * Verifies that the events hook subscribes only to INSERT events (not
   * UPDATE or DELETE) since events are append-only and never modified.
   */
  it("subscribes to INSERT events on agent_events", async () => {
    setupChain([], null);

    renderHook(() => useAgentEvents());

    await waitFor(() => {
      expect(mockOn).toHaveBeenCalledWith(
        "postgres_changes",
        { event: "INSERT", schema: "public", table: "agent_events" },
        expect.any(Function)
      );
    });
  });
});

// Tests the template listing hook for multi-step task definitions.
describe("useAgentTemplates", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /** Verifies template listing from the agent_templates table. */
  it("fetches templates on mount", async () => {
    const mockData = [
      { id: 1, name: "code-review", description: "Review code changes", steps: [], created_at: "2026-01-01", role_name: null },
    ];
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentTemplates());

    await waitFor(() => {
      expect(result.current.templates).toHaveLength(1);
    });
    expect(result.current.templates[0].name).toBe("code-review");
    expect(mockFrom).toHaveBeenCalledWith("agent_templates");
  });

  /** Verifies Realtime subscription for live template updates. */
  it("subscribes to realtime on agent_templates", async () => {
    setupChain([], null);

    renderHook(() => useAgentTemplates());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalledWith("agent_templates_changes");
    });
  });
});

// Tests the role listing hook for agent role configurations.
describe("useAgentRoles", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies role listing from the agent_roles table. Roles define
   * domain-specific agent configurations with default models, permissions,
   * and cost limits.
   */
  it("fetches roles on mount", async () => {
    const mockData = [
      { id: 1, name: "engine-analyst", description: "Analyzes engine", domains: ["engine"], default_permission_level: 1, default_model: "claude-opus-4-20250514", system_prompt: null, default_max_cost_usd: 5.0, created_at: "2026-01-01", updated_at: "2026-01-01" },
    ];
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentRoles());

    await waitFor(() => {
      expect(result.current.roles).toHaveLength(1);
    });
    expect(result.current.roles[0].name).toBe("engine-analyst");
    expect(mockFrom).toHaveBeenCalledWith("agent_roles");
  });
});

// Tests the agent logs hook which retrieves execution logs for a specific task
// via the REST API. Logs include stdout/stderr lines from agent processes.
describe("useAgentLogs", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that logs are fetched for a specific task ID via the REST API.
   * The response includes both the log entries and total count for pagination.
   */
  it("fetches logs for a given task", async () => {
    const mockLogs = [
      { id: 1, task_id: 1, stream: "stdout", line_num: 1, msg_type: null, content: "Hello", created_at: "2026-01-01" },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ logs: mockLogs, total: 1 }),
    });

    const { result } = renderHook(() => useAgentLogs(1));

    await waitFor(() => {
      expect(result.current.logs).toHaveLength(1);
    });
    expect(result.current.total).toBe(1);
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/tasks/1/logs"));
  });

  /**
   * Verifies that the hook does not issue a fetch when taskId is null.
   * This supports conditional rendering where a task may not be selected yet.
   */
  it("does not fetch when taskId is null", async () => {
    const { result } = renderHook(() => useAgentLogs(null));

    // Should stay in loading state without fetching
    expect(mockFetch).not.toHaveBeenCalled();
    expect(result.current.logs).toEqual([]);
  });

  /** Verifies graceful handling of network errors during log fetch. */
  it("handles fetch error gracefully", async () => {
    mockFetch.mockRejectedValue(new Error("Network error"));

    const { result } = renderHook(() => useAgentLogs(1));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.logs).toEqual([]);
  });
});

// Tests the agent timeline hook which provides a chronological view of
// task execution events (tool calls, token usage, durations) via REST API.
describe("useAgentTimeline", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies timeline event fetching for a specific task. Timeline events
   * include tool_name, token counts, and duration_ms for performance analysis.
   */
  it("fetches timeline events for a task", async () => {
    const mockEvents = [
      { id: 1, task_id: 1, event_type: "tool_call", agent: null, summary: "Called grep", detail: null, created_at: "2026-01-01", tool_name: "grep", input_tokens: 100, output_tokens: 50, duration_ms: 500 },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockEvents),
    });

    const { result } = renderHook(() => useAgentTimeline(1));

    await waitFor(() => {
      expect(result.current.events).toHaveLength(1);
    });
    expect(result.current.events[0].tool_name).toBe("grep");
    expect(mockFetch).toHaveBeenCalledWith("http://localhost:3000/api/agents/tasks/1/timeline");
  });

  /** Verifies that no fetch is issued when taskId is null. */
  it("does not fetch when taskId is null", async () => {
    renderHook(() => useAgentTimeline(null));

    expect(mockFetch).not.toHaveBeenCalled();
  });
});

// Tests the createTask function which inserts a new agent task into Supabase.
// Supports optional parameters for model, cost limits, permission level, and role.
describe("createTask", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies task creation with all optional fields specified: model,
   * cost limit, permission level, and role name. The source is always
   * set to "manual" for UI-created tasks.
   */
  it("creates a task with all fields", async () => {
    const returnData = makeTask({ id: 10, title: "New task" });
    const chain = {
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: returnData, error: null }),
    };
    mockInsert.mockReturnValue(chain);
    mockFrom.mockReturnValue({ insert: mockInsert });

    const result = await createTask("New task", "Description", "high", "claude-opus-4-20250514", 5.0, 2, "engine-analyst");

    expect(mockFrom).toHaveBeenCalledWith("agent_tasks");
    expect(mockInsert).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "New task",
        description: "Description",
        priority: "high",
        agent_model: "claude-opus-4-20250514",
        source: "manual",
        max_cost_usd: 5.0,
        permission_level: 2,
        role_name: "engine-analyst",
      })
    );
    expect(result.id).toBe(10);
  });

  /**
   * Verifies that optional fields default to null when not provided.
   * Permission level defaults to 1 (basic permissions).
   */
  it("sets null for optional fields when not provided", async () => {
    const returnData = makeTask({ id: 11 });
    const chain = {
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: returnData, error: null }),
    };
    mockInsert.mockReturnValue(chain);
    mockFrom.mockReturnValue({ insert: mockInsert });

    await createTask("Task", "Desc", "normal");

    expect(mockInsert).toHaveBeenCalledWith(
      expect.objectContaining({
        agent_model: null,
        max_cost_usd: null,
        permission_level: 1,
        role_name: null,
      })
    );
  });

  /** Verifies that createTask throws when the budget quota is exceeded. */
  it("throws on insert error", async () => {
    const chain = {
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: null, error: { message: "Quota exceeded" } }),
    };
    mockInsert.mockReturnValue(chain);
    mockFrom.mockReturnValue({ insert: mockInsert });

    await expect(createTask("t", "d", "low")).rejects.toEqual({ message: "Quota exceeded" });
  });
});

// Tests the cancelTask function which sets a task's status to "cancelled".
// The update includes a status guard: only "pending" or "in_progress" tasks
// can be cancelled (using .in("status", ["pending", "in_progress"])).
describe("cancelTask", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that cancelTask updates the status to "cancelled" with a guard
   * ensuring only pending or in_progress tasks can be cancelled. The .in()
   * filter prevents cancelling already-completed or failed tasks.
   */
  it("cancels a pending or in_progress task", async () => {
    mockIn.mockResolvedValue({ error: null });
    const chain = {
      eq: mockEq.mockReturnValue({ in: mockIn }),
      update: mockUpdate,
    };
    mockUpdate.mockReturnValue({ eq: mockEq });
    mockFrom.mockReturnValue({ update: mockUpdate });

    await cancelTask(42);

    expect(mockFrom).toHaveBeenCalledWith("agent_tasks");
    expect(mockUpdate).toHaveBeenCalledWith(
      expect.objectContaining({ status: "cancelled" })
    );
    expect(mockEq).toHaveBeenCalledWith("id", 42);
    expect(mockIn).toHaveBeenCalledWith("status", ["pending", "in_progress"]);
  });

  /** Verifies that cancelTask throws when the task is already completed. */
  it("throws on cancel error", async () => {
    mockIn.mockResolvedValue({ error: { message: "Already completed" } });
    mockEq.mockReturnValue({ in: mockIn });
    mockUpdate.mockReturnValue({ eq: mockEq });
    mockFrom.mockReturnValue({ update: mockUpdate });

    await expect(cancelTask(99)).rejects.toEqual({ message: "Already completed" });
  });
});

// Tests the expandTemplate function which calls the REST API to instantiate
// a multi-step template into a parent task with child sub-tasks.
describe("expandTemplate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that expandTemplate sends a POST request to the template
   * expansion endpoint and returns the parent_task_id from the response.
   */
  it("expands a template via REST API", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ parent_task_id: 100 }),
    });

    const parentId = await expandTemplate("code-review", "Review PR", "Check code", "high", 10.0, 2, "engine-analyst");

    expect(parentId).toBe(100);
    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:3000/api/agents/templates/code-review/expand",
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
      })
    );
  });

  /** Verifies that expandTemplate throws when the template is not found. */
  it("throws on failed expansion", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Template not found" }),
    });

    await expect(
      expandTemplate("nonexistent", "t", "d", "normal")
    ).rejects.toThrow("Template not found");
  });
});

// Tests the buildTaskTree pure function which transforms a flat list of tasks
// (with parent_task_id references) into a hierarchical tree structure for
// display in the task tree view component.
describe("buildTaskTree", () => {
  /**
   * Verifies tree construction from a flat task list with parent-child
   * relationships. Two root tasks (Parent and Standalone) should produce
   * two top-level nodes, with the Parent having two children.
   */
  it("builds tree from flat task list", () => {
    const tasks = [
      makeTask({ id: 1, title: "Parent", parent_task_id: null }),
      makeTask({ id: 2, title: "Child 1", parent_task_id: 1 }),
      makeTask({ id: 3, title: "Child 2", parent_task_id: 1 }),
      makeTask({ id: 4, title: "Standalone", parent_task_id: null }),
    ];

    const tree = buildTaskTree(tasks);

    expect(tree).toHaveLength(2);
    expect(tree[0].task.title).toBe("Parent");
    expect(tree[0].children).toHaveLength(2);
    expect(tree[1].task.title).toBe("Standalone");
    expect(tree[1].children).toHaveLength(0);
  });

  /** Verifies that an empty input produces an empty tree. */
  it("returns empty array for empty input", () => {
    expect(buildTaskTree([])).toEqual([]);
  });

  /**
   * Verifies that children are sorted by id (ascending) regardless of
   * input order. This ensures consistent display order in the UI.
   */
  it("sorts children by id", () => {
    const tasks = [
      makeTask({ id: 1, parent_task_id: null }),
      makeTask({ id: 5, title: "Later", parent_task_id: 1 }),
      makeTask({ id: 3, title: "Earlier", parent_task_id: 1 }),
    ];

    const tree = buildTaskTree(tasks);

    expect(tree[0].children[0].id).toBe(3);
    expect(tree[0].children[1].id).toBe(5);
  });

  /**
   * Verifies handling of orphaned tasks whose parent_task_id references
   * a non-existent parent (id=999). These orphans are excluded from the
   * tree since they have no root node to attach to.
   */
  it("handles all tasks being children (orphans)", () => {
    const tasks = [
      makeTask({ id: 2, parent_task_id: 999 }),
      makeTask({ id: 3, parent_task_id: 999 }),
    ];

    const tree = buildTaskTree(tasks);

    // No top-level tasks, so tree is empty (children are skipped)
    expect(tree).toHaveLength(0);
  });

  /** Verifies that a single root task with no children has an empty children array. */
  it("handles single task with no children", () => {
    const tasks = [makeTask({ id: 1, parent_task_id: null })];

    const tree = buildTaskTree(tasks);

    expect(tree).toHaveLength(1);
    expect(tree[0].children).toHaveLength(0);
  });
});
