/**
 * @file Tests for useAgentMemory hook, upsertMemory, deleteMemory, and MEMORY_CATEGORIES
 * @module __tests__/hooks/use-agent-memory
 *
 * Validates the agent memory key-value store system used for persistent learning.
 * Agents store conventions, patterns, gotchas, and architectural decisions as
 * structured memories that persist across sessions. Tests cover the full CRUD
 * lifecycle (fetch, upsert, delete) via the REST API.
 *
 * @see {@link ../../hooks/use-agent-memory} Source hook
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// --- fetch mock ---
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import { useAgentMemory, upsertMemory, deleteMemory, MEMORY_CATEGORIES } from "@/hooks/use-agent-memory";

// Tests the memory data fetching lifecycle via REST API.
// Validates that the hook queries the /api/agents/memory endpoint
// and handles network failures gracefully.
describe("useAgentMemory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches memory entries on mount and exposes them
   * through the returned memories array. The mock simulates a successful
   * REST response returning one convention-type memory ("rust-style").
   *
   * Assertions: memories array populated, key and category match expected
   * values, loading completes, correct endpoint queried.
   */
  it("fetches memories on mount", async () => {
    const mockData = [
      {
        id: 1,
        key: "rust-style",
        value: "Use snake_case for functions",
        category: "convention",
        created_by_task: null,
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(result.current.memories).toHaveLength(1);
    });
    expect(result.current.memories[0].key).toBe("rust-style");
    expect(result.current.memories[0].category).toBe("convention");
    expect(result.current.loading).toBe(false);
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/memory"));
  });

  /**
   * Verifies graceful error handling when the REST API returns a non-ok response.
   * The hook should return an empty array rather than throw.
   */
  it("returns empty array on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Table not found" }),
    });

    const { result } = renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.memories).toEqual([]);
  });

  /**
   * Verifies the initial loading state before async fetch completes.
   */
  it("starts with loading true", () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { result } = renderHook(() => useAgentMemory());

    expect(result.current.loading).toBe(true);
  });

  /**
   * Verifies that the hook exposes a refetch function for manual re-fetch
   * after memory mutations.
   */
  it("provides a refetch function", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { result } = renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies graceful handling of network-level fetch failures.
   * The hook should return an empty array and set loading to false.
   */
  it("returns empty array on network error", async () => {
    mockFetch.mockRejectedValue(new Error("Network error"));

    const { result } = renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.memories).toEqual([]);
  });
});

// Tests the upsertMemory standalone function which creates or updates a memory
// entry by key via PUT /api/agents/memory.
describe("upsertMemory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that upsertMemory sends a PUT request with the correct body
   * and returns the upserted memory entry.
   */
  it("upserts a memory with default category", async () => {
    const returnData = {
      id: 1,
      key: "test-key",
      value: "test-value",
      category: "general",
      created_by_task: null,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    };
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(returnData),
    });

    const result = await upsertMemory("test-key", "test-value");

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/agents/memory"),
      expect.objectContaining({
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key: "test-key", value: "test-value", category: "general" }),
      })
    );
    expect(result.key).toBe("test-key");
  });

  /**
   * Verifies that upsertMemory passes a custom category when specified.
   */
  it("upserts a memory with custom category", async () => {
    const returnData = {
      id: 2,
      key: "gotcha-key",
      value: "gotcha-value",
      category: "gotcha",
      created_by_task: null,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    };
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(returnData),
    });

    const result = await upsertMemory("gotcha-key", "gotcha-value", "gotcha");

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/agents/memory"),
      expect.objectContaining({
        body: JSON.stringify({ key: "gotcha-key", value: "gotcha-value", category: "gotcha" }),
      })
    );
    expect(result.category).toBe("gotcha");
  });

  /**
   * Verifies that upsertMemory throws an error when the REST API
   * returns a non-ok response.
   */
  it("throws on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Conflict" }),
    });

    await expect(upsertMemory("key", "val")).rejects.toThrow("Conflict");
  });
});

// Tests the deleteMemory standalone function which removes a memory entry
// by its unique key via DELETE /api/agents/memory/:key.
describe("deleteMemory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that deleteMemory sends a DELETE request to the correct endpoint.
   */
  it("deletes a memory by key", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({}),
    });

    await deleteMemory("test-key");

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/agents/memory/test-key"),
      expect.objectContaining({ method: "DELETE" })
    );
  });

  /**
   * Verifies that deleteMemory throws an error when the REST API
   * returns a non-ok response.
   */
  it("throws on delete error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Not found" }),
    });

    await expect(deleteMemory("nonexistent")).rejects.toThrow("Not found");
  });
});

// Tests the MEMORY_CATEGORIES constant which defines the taxonomy of memory types.
// These categories are used to organize agent knowledge by domain.
describe("MEMORY_CATEGORIES", () => {
  /**
   * Verifies that MEMORY_CATEGORIES exports exactly the 6 known categories:
   * pattern, convention, gotcha, preference, architecture, and general.
   * Adding or removing categories would require updating this test.
   */
  it("exports known categories", () => {
    expect(MEMORY_CATEGORIES).toContain("pattern");
    expect(MEMORY_CATEGORIES).toContain("convention");
    expect(MEMORY_CATEGORIES).toContain("gotcha");
    expect(MEMORY_CATEGORIES).toContain("preference");
    expect(MEMORY_CATEGORIES).toContain("architecture");
    expect(MEMORY_CATEGORIES).toContain("general");
    expect(MEMORY_CATEGORIES).toHaveLength(6);
  });
});
