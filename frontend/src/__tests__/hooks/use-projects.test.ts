/**
 * @file Tests for useProjects hook
 * @module __tests__/hooks/use-projects
 *
 * Validates the project campaign data hook which lists active and completed
 * search campaigns from the "projects" table. Projects represent organized
 * multi-search campaigns with specific objectives (e.g., "find a 1M-digit
 * factorial prime") and track aggregated metrics like total_tested,
 * total_found, and best_digits.
 *
 * The hook supports Supabase Realtime subscriptions for live project
 * status updates as search results come in.
 *
 * @see {@link ../../hooks/use-projects} Source hook
 * @see {@link ../../components/project-card} Project display component
 * @see {@link ../../app/projects/page} Projects page
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock supabase query chain with limit for bounded project lists
const mockSelect = vi.fn();
const mockOrder = vi.fn();
const mockLimit = vi.fn();
const mockFrom = vi.fn();
const mockChannel = vi.fn();
const mockOn = vi.fn();
const mockSubscribe = vi.fn();
const mockRemoveChannel = vi.fn();

/**
 * Configures the mock chain for project queries.
 * Chain ends with .limit() to bound the number of returned projects.
 */
function setupChain(finalData: unknown, finalError: unknown) {
  const chain = {
    select: mockSelect.mockReturnThis(),
    order: mockOrder.mockReturnThis(),
    limit: mockLimit.mockResolvedValue({
      data: finalData,
      error: finalError,
    }),
  };
  mockFrom.mockReturnValue(chain);
  return chain;
}

// Realtime channel mock for live project status updates
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

import { useProjects } from "@/hooks/use-projects";

// Tests the project listing lifecycle: mount -> loading -> data/error -> Realtime.
describe("useProjects", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies that the hook fetches projects on mount from the "projects" table.
   * The mock returns one active factorial campaign with 50K tested and 12 found.
   */
  it("fetches projects on mount", async () => {
    const mockData = [
      {
        id: 1,
        slug: "factorial-million",
        name: "Factorial Million",
        description: "Search factorial primes up to 1M",
        objective: "record",
        form: "factorial",
        status: "active",
        total_tested: 50000,
        total_found: 12,
        best_digits: 45678,
        total_cost_usd: 3.25,
        created_at: "2026-01-01T00:00:00Z",
        started_at: "2026-01-02T00:00:00Z",
        completed_at: null,
      },
    ];
    setupChain(mockData, null);

    const { result } = renderHook(() => useProjects());

    await waitFor(() => {
      expect(result.current.projects).toHaveLength(1);
    });
    expect(result.current.projects[0].name).toBe("Factorial Million");
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(mockFrom).toHaveBeenCalledWith("projects");
  });

  /**
   * Verifies graceful error handling; projects defaults to empty array
   * and the error message is exposed via result.current.error.
   */
  it("returns empty array on error", async () => {
    setupChain(null, { message: "Database error" });

    const { result } = renderHook(() => useProjects());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.projects).toEqual([]);
    expect(result.current.error).toBe("Database error");
  });

  /** Verifies that a manual refetch function is exposed. */
  it("provides a refetch function", async () => {
    setupChain([], null);

    const { result } = renderHook(() => useProjects());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies Realtime subscription to all events on the projects table
   * for live campaign progress updates.
   */
  it("subscribes to realtime changes", async () => {
    setupChain([], null);

    renderHook(() => useProjects());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalledWith("projects_changes");
    });
    expect(mockOn).toHaveBeenCalledWith(
      "postgres_changes",
      { event: "*", schema: "public", table: "projects" },
      expect.any(Function)
    );
    expect(mockSubscribe).toHaveBeenCalled();
  });
});
