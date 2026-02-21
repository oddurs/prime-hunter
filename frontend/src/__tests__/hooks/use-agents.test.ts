/**
 * @file Tests for the use-agents hook (aggregate agent data hook)
 * @module __tests__/hooks/use-agents
 *
 * Tests the consolidated agent hooks exported from use-agents.ts, which
 * provides a unified API for agent task listing, role management, and
 * task tree construction. This is the primary hook consumed by the
 * agents page component, combining data from REST API endpoints
 * with polling for live updates.
 *
 * @see {@link ../../hooks/use-agents} Source hook (barrel re-export)
 * @see {@link ../../hooks/use-agent-tasks} Underlying implementation
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// --- fetch mock ---
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import { useAgentTasks, useAgentRoles, buildTaskTree } from "@/hooks/use-agents";
import type { AgentTask } from "@/hooks/use-agents";

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

// Tests the consolidated agent task listing hook with polling.
describe("useAgentTasks", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ tasks: mockData }),
    });

    const { result } = renderHook(() => useAgentTasks());

    await waitFor(() => {
      expect(result.current.tasks).toHaveLength(1);
    });
    expect(result.current.tasks[0].title).toBe("Analyze factorial search");
    expect(result.current.loading).toBe(false);
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/tasks"));
  });

  /** Verifies graceful error handling; tasks defaults to empty array. */
  it("returns empty on error", async () => {
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
   * Verifies that the fetch URL includes limit=200 to control
   * the amount of data loaded.
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
});

// Tests the agent role listing hook from the consolidated use-agents module.
describe("useAgentRoles", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies role listing via the REST API. Each role defines
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
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ roles: mockData }),
    });

    const { result } = renderHook(() => useAgentRoles());

    await waitFor(() => {
      expect(result.current.roles).toHaveLength(1);
    });
    expect(result.current.roles[0].name).toBe("engine-analyst");
    expect(result.current.loading).toBe(false);
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/roles"));
  });

  /** Verifies graceful error handling when the roles endpoint fails. */
  it("returns empty on error", async () => {
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
