/**
 * @file Tests for useProjects and useProject hooks
 * @module __tests__/hooks/use-projects
 *
 * Validates the project campaign data hooks which list and detail active
 * and completed search campaigns from the REST API. Projects represent
 * organized multi-search campaigns with specific objectives (e.g., "find
 * a 1M-digit factorial prime") and track aggregated metrics like
 * total_tested, total_found, and best_digits.
 *
 * The hooks use `fetch()` to call:
 * - GET /api/projects (list all projects)
 * - GET /api/projects/:slug (project detail with phases)
 * - GET /api/projects/:slug/events (project events)
 *
 * Polling is done via setInterval (every 10 seconds) instead of Supabase Realtime.
 *
 * @see {@link ../../hooks/use-projects} Source hook
 * @see {@link ../../components/project-card} Project display component
 * @see {@link ../../app/projects/page} Projects page
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import { useProjects, useProject, activateProject, pauseProject, cancelProject } from "@/hooks/use-projects";

describe("useProjects", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockFetch = vi.fn();
    vi.stubGlobal("fetch", mockFetch);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  /**
   * Verifies that the hook fetches projects on mount from the REST API.
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
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ projects: mockData }),
    });

    const { result } = renderHook(() => useProjects());

    await waitFor(() => {
      expect(result.current.projects).toHaveLength(1);
    });
    expect(result.current.projects[0].name).toBe("Factorial Million");
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/projects")
    );
  });

  /**
   * Verifies graceful error handling; projects defaults to empty array
   * and the error message is exposed via result.current.error.
   */
  it("returns empty array on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useProjects());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.projects).toEqual([]);
    expect(result.current.error).toContain("Failed to fetch projects");
  });

  /** Verifies that a manual refetch function is exposed. */
  it("provides a refetch function", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ projects: [] }),
    });

    const { result } = renderHook(() => useProjects());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies that projects are polled every 10 seconds via setInterval
   * (replaces the old Supabase Realtime subscription).
   */
  it("polls for projects every 10 seconds", async () => {
    vi.useFakeTimers();
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ projects: [] }),
    });

    renderHook(() => useProjects());

    // Flush the initial useEffect + fetch
    await vi.advanceTimersByTimeAsync(0);
    expect(mockFetch).toHaveBeenCalledTimes(1);

    // Advance timer by 10 seconds to trigger polling
    await vi.advanceTimersByTimeAsync(10_000);
    expect(mockFetch).toHaveBeenCalledTimes(2);

    // Advance another 10 seconds
    await vi.advanceTimersByTimeAsync(10_000);
    expect(mockFetch).toHaveBeenCalledTimes(3);

    vi.useRealTimers();
  });
});

describe("useProject", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockFetch = vi.fn();
    vi.stubGlobal("fetch", mockFetch);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  /**
   * Verifies that useProject fetches the project detail and events on mount.
   * Two fetch calls are made: one for the project (with phases), one for events.
   */
  it("fetches project detail and events on mount", async () => {
    const projectData = {
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
      phases: [
        {
          id: 1,
          name: "Phase 1",
          description: "Initial search",
          phase_order: 1,
          status: "active",
          search_params: null,
          block_size: 1000,
          total_tested: 50000,
          total_found: 12,
          search_job_id: null,
          started_at: "2026-01-02T00:00:00Z",
          completed_at: null,
        },
      ],
    };
    const eventsData = {
      events: [
        {
          id: 1,
          project_id: 1,
          event_type: "milestone",
          summary: "Found first prime",
          detail: null,
          created_at: "2026-01-03T00:00:00Z",
        },
      ],
    };

    mockFetch
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(projectData),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(eventsData),
      });

    const { result } = renderHook(() => useProject("factorial-million"));

    await waitFor(() => {
      expect(result.current.project).not.toBeNull();
    });
    expect(result.current.project!.name).toBe("Factorial Million");
    expect(result.current.phases).toHaveLength(1);
    expect(result.current.events).toHaveLength(1);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();

    // Verify correct URLs were called
    const calls = mockFetch.mock.calls;
    expect(calls[0][0]).toContain("/api/projects/factorial-million");
    expect(calls[1][0]).toContain("/api/projects/factorial-million/events");
  });

  /** Verifies graceful error handling for project detail fetch. */
  it("handles error on project detail fetch", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 404,
    });

    const { result } = renderHook(() => useProject("nonexistent"));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.project).toBeNull();
    expect(result.current.error).toContain("Failed to fetch project");
  });

  /** Verifies that an empty slug skips fetching. */
  it("does not fetch when slug is empty", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ projects: [] }),
    });

    const { result } = renderHook(() => useProject(""));

    // Should not make any fetch calls
    expect(mockFetch).not.toHaveBeenCalled();
    expect(result.current.project).toBeNull();
  });
});

describe("project action functions", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockFetch = vi.fn();
    vi.stubGlobal("fetch", mockFetch);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  /** Verifies that activateProject sends POST to the activate endpoint. */
  it("activateProject sends POST", async () => {
    mockFetch.mockResolvedValue({ ok: true });

    await activateProject("factorial-million");

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/projects/factorial-million/activate"),
      expect.objectContaining({ method: "POST" })
    );
  });

  /** Verifies that pauseProject sends POST to the pause endpoint. */
  it("pauseProject sends POST", async () => {
    mockFetch.mockResolvedValue({ ok: true });

    await pauseProject("factorial-million");

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/projects/factorial-million/pause"),
      expect.objectContaining({ method: "POST" })
    );
  });

  /** Verifies that cancelProject sends POST to the cancel endpoint. */
  it("cancelProject sends POST", async () => {
    mockFetch.mockResolvedValue({ ok: true });

    await cancelProject("factorial-million");

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/projects/factorial-million/cancel"),
      expect.objectContaining({ method: "POST" })
    );
  });

  /** Verifies that action functions throw on error response. */
  it("action functions throw on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Project not found" }),
    });

    await expect(activateProject("bad-slug")).rejects.toThrow("Project not found");
  });
});
