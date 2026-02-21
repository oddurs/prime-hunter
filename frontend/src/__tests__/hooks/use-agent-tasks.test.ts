/**
 * @file Tests for useAgentTasks hook and related agent infrastructure hooks/functions
 * @module __tests__/hooks/use-agent-tasks
 *
 * Comprehensive test suite for the agent task management system. Covers:
 * - useAgentTasks: Task listing with status filtering and polling
 * - useAgentEvents: Event stream for agent activity (tool calls, completions)
 * - useAgentTemplates: Multi-step task template definitions
 * - useAgentRoles: Role-based agent configurations (domains, permissions, models)
 * - useAgentLogs: Task execution log retrieval via REST API
 * - useAgentTimeline: Task timeline events via REST API
 * - createTask: POST to REST API for new agent tasks
 * - cancelTask: POST to cancel endpoint with guard against completed tasks
 * - expandTemplate: REST API template expansion into parent+child task tree
 * - buildTaskTree: Pure function to transform flat task list into hierarchical tree
 *
 * All hooks now use fetch() with polling instead of Supabase client queries
 * and Realtime subscriptions.
 *
 * @see {@link ../../hooks/use-agent-tasks} Source hooks and functions
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// --- fetch mock ---
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

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

// Tests the task listing hook: mount -> loading -> data -> filtering -> polling.
// Validates fetch URL construction with optional status filter and limit parameter.
describe("useAgentTasks", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches agent tasks on mount via the REST API
   * and exposes them through the returned tasks array.
   */
  it("fetches tasks on mount", async () => {
    const mockData = [makeTask({ title: "Analyze search" })];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ tasks: mockData }),
    });

    const { result } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(result.current.tasks).toHaveLength(1);
    });
    expect(result.current.tasks[0].title).toBe("Analyze search");
    expect(result.current.loading).toBe(false);
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/tasks"));
  });

  /** Verifies graceful error handling; tasks defaults to empty array. */
  it("returns empty array on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.tasks).toEqual([]);
  });

  /**
   * Verifies that passing a status string (e.g., "in_progress") to the hook
   * appends a status query parameter to the fetch URL.
   */
  it("applies status filter when provided", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ tasks: [] }),
    });

    renderHook(() => useAgentTasks("in_progress"));

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining("status=in_progress")
      );
    });
  });

  /**
   * Verifies that omitting the status parameter does NOT add a status filter
   * to the query string. Only the limit parameter should be present.
   */
  it("does not apply status filter when undefined", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ tasks: [] }),
    });

    renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });

    const url = mockFetch.mock.calls[0][0] as string;
    expect(url).not.toContain("status=");
  });

  /**
   * Verifies that the fetch URL includes limit=200 to avoid loading
   * excessive data.
   */
  it("includes limit=200 in fetch URL", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ tasks: [] }),
    });

    renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining("limit=200")
      );
    });
  });

  /** Verifies polling interval cleanup on unmount. */
  it("cleans up polling on unmount", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ tasks: [] }),
    });

    const { unmount } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });

    unmount();
    // Unmount cleans up the interval via clearInterval in the useEffect cleanup.
    // No errors thrown confirms proper cleanup.
  });
});

// Tests the agent events hook which provides an activity stream of agent actions
// (tool calls, completions, errors). Supports optional filtering by task ID.
describe("useAgentEvents", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches all events when no taskId filter is provided.
   * Events include agent lifecycle events (started, completed) and tool calls.
   */
  it("fetches events on mount without taskId filter", async () => {
    const mockData = [
      { id: 1, task_id: null, event_type: "agent_started", agent: "agent-1", summary: "Agent started", detail: null, created_at: "2026-01-01", tool_name: null, input_tokens: null, output_tokens: null, duration_ms: null },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ events: mockData }),
    });

    const { result } = renderHook(() => useAgentEvents());

    await waitFor(() => {
      expect(result.current.events).toHaveLength(1);
    });
    expect(result.current.events[0].event_type).toBe("agent_started");
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/events"));
  });

  /**
   * Verifies that passing a taskId filters events by including task_id
   * as a query parameter in the fetch URL.
   */
  it("filters events by taskId when provided", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ events: [] }),
    });

    renderHook(() => useAgentEvents(42));

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining("task_id=42")
      );
    });
  });

  /**
   * Verifies that the events hook includes limit=200 in the fetch URL.
   */
  it("includes limit=200 in fetch URL", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ events: [] }),
    });

    renderHook(() => useAgentEvents());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining("limit=200")
      );
    });
  });
});

// Tests the template listing hook for multi-step task definitions.
describe("useAgentTemplates", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /** Verifies template listing via the REST API. */
  it("fetches templates on mount", async () => {
    const mockData = [
      { id: 1, name: "code-review", description: "Review code changes", steps: [], created_at: "2026-01-01", role_name: null },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ templates: mockData }),
    });

    const { result } = renderHook(() => useAgentTemplates());

    await waitFor(() => {
      expect(result.current.templates).toHaveLength(1);
    });
    expect(result.current.templates[0].name).toBe("code-review");
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/templates"));
  });

  /** Verifies graceful error handling; templates defaults to empty array. */
  it("returns empty array on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useAgentTemplates());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.templates).toEqual([]);
  });
});

// Tests the role listing hook for agent role configurations.
describe("useAgentRoles", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies role listing via the REST API. Roles define
   * domain-specific agent configurations with default models, permissions,
   * and cost limits.
   */
  it("fetches roles on mount", async () => {
    const mockData = [
      { id: 1, name: "engine-analyst", description: "Analyzes engine", domains: ["engine"], default_permission_level: 1, default_model: "claude-opus-4-20250514", system_prompt: null, default_max_cost_usd: 5.0, created_at: "2026-01-01", updated_at: "2026-01-01" },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ roles: mockData }),
    });

    const { result } = renderHook(() => useAgentRoles());

    await waitFor(() => {
      expect(result.current.roles).toHaveLength(1);
    });
    expect(result.current.roles[0].name).toBe("engine-analyst");
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/roles"));
  });

  /** Verifies graceful error handling; roles defaults to empty array. */
  it("returns empty array on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useAgentRoles());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.roles).toEqual([]);
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
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/tasks/1/timeline"));
  });

  /** Verifies that no fetch is issued when taskId is null. */
  it("does not fetch when taskId is null", async () => {
    renderHook(() => useAgentTimeline(null));

    expect(mockFetch).not.toHaveBeenCalled();
  });
});

// Tests the createTask function which sends a POST request to create a new agent task.
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
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(returnData),
    });

    const result = await createTask("New task", "Description", "high", "claude-opus-4-20250514", 5.0, 2, "engine-analyst");

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/agents/tasks"),
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
      })
    );

    const callArgs = mockFetch.mock.calls[0];
    const body = JSON.parse(callArgs[1].body);
    expect(body.title).toBe("New task");
    expect(body.description).toBe("Description");
    expect(body.priority).toBe("high");
    expect(body.agent_model).toBe("claude-opus-4-20250514");
    expect(body.source).toBe("manual");
    expect(body.max_cost_usd).toBe(5.0);
    expect(body.permission_level).toBe(2);
    expect(body.role_name).toBe("engine-analyst");

    expect(result.id).toBe(10);
  });

  /**
   * Verifies that optional fields default to null when not provided.
   * Permission level defaults to 1 (basic permissions).
   */
  it("sets null for optional fields when not provided", async () => {
    const returnData = makeTask({ id: 11 });
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(returnData),
    });

    await createTask("Task", "Desc", "normal");

    const callArgs = mockFetch.mock.calls[0];
    const body = JSON.parse(callArgs[1].body);
    expect(body.agent_model).toBeNull();
    expect(body.max_cost_usd).toBeNull();
    expect(body.permission_level).toBe(1);
    expect(body.role_name).toBeNull();
  });

  /** Verifies that createTask throws when the API returns an error. */
  it("throws on insert error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Quota exceeded" }),
    });

    await expect(createTask("t", "d", "low")).rejects.toThrow("Quota exceeded");
  });
});

// Tests the cancelTask function which posts to the cancel endpoint.
describe("cancelTask", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that cancelTask sends a POST to the cancel endpoint
   * with the correct task ID.
   */
  it("cancels a task by id", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
    });

    await cancelTask(42);

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/agents/tasks/42/cancel"),
      expect.objectContaining({
        method: "POST",
      })
    );
  });

  /** Verifies that cancelTask throws when the task cannot be cancelled. */
  it("throws on cancel error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Already completed" }),
    });

    await expect(cancelTask(99)).rejects.toThrow("Already completed");
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
      expect.stringContaining("/api/agents/templates/code-review/expand"),
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
