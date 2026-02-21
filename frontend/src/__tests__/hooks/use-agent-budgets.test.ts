/**
 * @file Tests for useAgentBudgets, useAgentDailyCosts, useAgentTemplateCosts, and useAgentAnomalies hooks
 * @module __tests__/hooks/use-agent-budgets
 *
 * Validates budget tracking, daily cost aggregation, template cost breakdown, and
 * anomaly detection for the AI agent cost control system. Tests cover the full budget
 * lifecycle: fetch on mount, loading states, error handling, refetch, and multi-period
 * budget display. All hooks fetch data via the REST API using the global fetch mock.
 *
 * @see {@link ../../hooks/use-agent-budgets} Source hooks
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// --- fetch mock for all REST API hooks ---
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import {
  useAgentBudgets,
  useAgentDailyCosts,
  useAgentTemplateCosts,
  useAgentAnomalies,
} from "@/hooks/use-agent-budgets";

// Tests the budget data fetching lifecycle: mount -> loading -> data -> error states.
// Validates that the hook correctly queries the REST API `/api/agents/budgets`
// endpoint and handles network failures gracefully.
describe("useAgentBudgets", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches budget data on mount and exposes it
   * through the returned budgets array. The mock simulates a successful
   * REST response returning one daily budget period with $10.00 allocated
   * and $3.50 spent.
   *
   * Assertions: budgets array contains the entry, period matches, loading
   * completes, and the correct endpoint was called.
   */
  it("fetches budgets on mount", async () => {
    const mockData = [
      {
        id: 1,
        period: "daily",
        budget_usd: 10.0,
        spent_usd: 3.5,
        tokens_used: 50000,
        period_start: "2026-02-01T00:00:00Z",
        updated_at: "2026-02-01T12:00:00Z",
      },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useAgentBudgets());

    await waitFor(() => {
      expect(result.current.budgets).toHaveLength(1);
    });
    expect(result.current.budgets[0].period).toBe("daily");
    expect(result.current.budgets[0].budget_usd).toBe(10.0);
    expect(result.current.loading).toBe(false);
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/budgets"));
  });

  /**
   * Verifies graceful error handling when the REST API returns a non-ok response.
   *
   * Assertion: budgets defaults to an empty array rather than throwing,
   * and loading transitions to false.
   */
  it("returns empty array on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Permission denied" }),
    });

    const { result } = renderHook(() => useAgentBudgets());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.budgets).toEqual([]);
  });

  /**
   * Verifies the initial loading state before any async operations complete.
   * On first render, the hook should indicate loading=true so the UI
   * can show a skeleton or spinner.
   */
  it("starts with loading true", () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { result } = renderHook(() => useAgentBudgets());

    // On first render, loading should be true
    expect(result.current.loading).toBe(true);
  });

  /**
   * Verifies that the hook exposes a refetch function for manual data refresh.
   * This allows the UI to trigger a re-fetch after budget mutations (e.g.,
   * adjusting limits or resetting periods).
   */
  it("provides a refetch function", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { result } = renderHook(() => useAgentBudgets());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies that the hook correctly handles multiple budget periods
   * (daily, weekly, monthly) returned from the API. Ensures all
   * entries are preserved in order and accessible by index.
   */
  it("handles multiple budgets", async () => {
    const mockData = [
      { id: 1, period: "daily", budget_usd: 10, spent_usd: 3, tokens_used: 50000, period_start: "2026-02-01", updated_at: "2026-02-01" },
      { id: 2, period: "weekly", budget_usd: 50, spent_usd: 20, tokens_used: 200000, period_start: "2026-02-01", updated_at: "2026-02-01" },
      { id: 3, period: "monthly", budget_usd: 200, spent_usd: 80, tokens_used: 800000, period_start: "2026-02-01", updated_at: "2026-02-01" },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useAgentBudgets());

    await waitFor(() => {
      expect(result.current.budgets).toHaveLength(3);
    });
    expect(result.current.budgets[0].period).toBe("daily");
    expect(result.current.budgets[2].period).toBe("monthly");
  });

  /**
   * Verifies graceful handling of network-level fetch failures.
   * The hook should return an empty array and set loading to false.
   */
  it("returns empty array on network error", async () => {
    mockFetch.mockRejectedValue(new Error("Network error"));

    const { result } = renderHook(() => useAgentBudgets());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.budgets).toEqual([]);
  });
});

// Tests the REST API-backed daily cost aggregation hook.
// Validates fetch URL construction with the days parameter,
// successful data parsing, and error/non-ok response handling.
describe("useAgentDailyCosts", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches daily cost data on mount with an explicit
   * days parameter (7). The mock simulates a successful REST response with
   * one day of cost data for the Claude Opus model.
   *
   * Assertions: data array populated, model field matches, correct URL called
   * with days=7 query parameter.
   */
  it("fetches daily costs on mount", async () => {
    const mockData = [
      { date: "2026-02-01", model: "claude-opus-4-20250514", total_cost: 5.0, total_tokens: 100000, task_count: 10 },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useAgentDailyCosts(7));

    await waitFor(() => {
      expect(result.current.data).toHaveLength(1);
    });
    expect(result.current.data[0].model).toBe("claude-opus-4-20250514");
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/analytics/daily-costs?days=7"));
  });

  /**
   * Verifies graceful handling of network-level fetch failures.
   * The mock rejects with a network error, and the hook should
   * return an empty array rather than propagating the exception.
   */
  it("returns empty array on fetch error", async () => {
    mockFetch.mockRejectedValue(new Error("Network error"));

    const { result } = renderHook(() => useAgentDailyCosts());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.data).toEqual([]);
  });

  /**
   * Verifies handling of HTTP error responses (non-2xx status codes).
   * Even when the server responds (no network error), a non-ok response
   * should result in an empty data array.
   */
  it("returns empty array on non-ok response", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Server error" }),
    });

    const { result } = renderHook(() => useAgentDailyCosts());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.data).toEqual([]);
  });

  /**
   * Verifies that the hook defaults to 30 days when no explicit days
   * parameter is provided. This ensures the URL includes days=30.
   */
  it("uses default days parameter of 30", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    renderHook(() => useAgentDailyCosts());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/analytics/daily-costs?days=30"));
    });
  });
});

// Tests the template cost breakdown hook which aggregates costs by agent template
// (e.g., "code-review", "search-analysis"). Validates fetch behavior and error handling.
describe("useAgentTemplateCosts", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches per-template cost aggregation on mount.
   * The mock returns a single template ("code-review") with accumulated
   * task count, total cost, and average cost metrics.
   *
   * Assertions: data array populated, template_name matches, correct API
   * endpoint called.
   */
  it("fetches template costs on mount", async () => {
    const mockData = [
      { template_name: "code-review", task_count: 15, total_cost: 7.5, avg_cost: 0.5, total_tokens: 300000, avg_tokens: 20000 },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useAgentTemplateCosts());

    await waitFor(() => {
      expect(result.current.data).toHaveLength(1);
    });
    expect(result.current.data[0].template_name).toBe("code-review");
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/analytics/template-costs"));
  });

  /**
   * Verifies that a network-level fetch failure results in an empty
   * data array rather than an unhandled exception.
   */
  it("returns empty array on fetch failure", async () => {
    mockFetch.mockRejectedValue(new Error("Network error"));

    const { result } = renderHook(() => useAgentTemplateCosts());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.data).toEqual([]);
  });
});

// Tests the anomaly detection hook which identifies agent tasks with
// unusually high token usage or cost (statistical outliers). Validates
// default and custom threshold parameters and error handling.
describe("useAgentAnomalies", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches anomaly data with the default threshold
   * of 3 (standard deviations). The mock returns one anomalous task with
   * 500K tokens and $15.00 cost.
   *
   * Assertion: correct API endpoint called with threshold=3 query parameter.
   */
  it("fetches anomalies with default threshold", async () => {
    const mockData = [
      { id: 42, title: "Runaway task", tokens_used: 500000, cost_usd: 15.0 },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useAgentAnomalies());

    await waitFor(() => {
      expect(result.current.data).toHaveLength(1);
    });
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/analytics/anomalies?threshold=3"));
  });

  /**
   * Verifies that a custom threshold value (5) is correctly passed
   * as a query parameter to the anomalies endpoint.
   */
  it("passes custom threshold", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    renderHook(() => useAgentAnomalies(5));

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/agents/analytics/anomalies?threshold=5"));
    });
  });

  /**
   * Verifies graceful handling of a network timeout when fetching anomalies.
   * The hook should return an empty array and set loading to false.
   */
  it("returns empty array on network error", async () => {
    mockFetch.mockRejectedValue(new Error("Timeout"));

    const { result } = renderHook(() => useAgentAnomalies());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.data).toEqual([]);
  });
});
