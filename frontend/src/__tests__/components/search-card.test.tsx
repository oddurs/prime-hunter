/**
 * @file Tests for the SearchCard component
 * @module __tests__/components/search-card
 *
 * Validates the search card that displays a managed search process on the
 * Searches page. Tests cover search type badge, status label, search ID,
 * parameter rendering for kbn/factorial/palindromic forms, tested/found
 * stats, status-dependent action buttons (Pause/Resume/Cancel), completed
 * state (no buttons), and failed status with error message display.
 *
 * @see {@link ../../components/search-card} Source component
 * @see {@link ../../hooks/use-websocket} ManagedSearch type
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SearchCard } from "@/components/search-card";
import type { ManagedSearch } from "@/hooks/use-websocket";

// Mock fetch for button handlers (Pause, Resume, Cancel API calls).
vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true }));

function makeSearch(overrides: Partial<ManagedSearch> = {}): ManagedSearch {
  return {
    id: 1,
    search_type: "kbn",
    params: { search_type: "kbn", k: 3, base: 2, min_n: 1, max_n: 1000 },
    status: "running",
    started_at: "2026-01-15T12:00:00Z",
    stopped_at: null,
    pid: 12345,
    worker_id: "worker-1",
    tested: 500,
    found: 3,
    ...overrides,
  };
}

// Tests the SearchCard: type badge, status, ID, form-specific params,
// stats, action buttons per status, and error message display.
describe("SearchCard", () => {
  it("renders search type badge", () => {
    render(<SearchCard search={makeSearch()} />);
    expect(screen.getByText("kbn")).toBeInTheDocument();
  });

  it("renders Running status", () => {
    render(<SearchCard search={makeSearch()} />);
    expect(screen.getByText("Running")).toBeInTheDocument();
  });

  it("renders search id", () => {
    render(<SearchCard search={makeSearch({ id: 42 })} />);
    expect(screen.getByText("#42")).toBeInTheDocument();
  });

  it("renders kbn parameters", () => {
    render(<SearchCard search={makeSearch()} />);
    expect(screen.getByText("k=3, base=2, n=1..1,000")).toBeInTheDocument();
  });

  it("renders factorial parameters", () => {
    const s = makeSearch({
      search_type: "factorial",
      params: { search_type: "factorial", start: 1, end: 100 },
    });
    render(<SearchCard search={s} />);
    expect(screen.getByText("n=1..100")).toBeInTheDocument();
  });

  it("renders palindromic parameters", () => {
    const s = makeSearch({
      search_type: "palindromic",
      params: { search_type: "palindromic", base: 10, min_digits: 1, max_digits: 9 },
    });
    render(<SearchCard search={s} />);
    expect(screen.getByText("base 10, 1..9 digits")).toBeInTheDocument();
  });

  it("shows stats when available", () => {
    render(<SearchCard search={makeSearch({ tested: 500, found: 3 })} />);
    expect(screen.getByText("3 found")).toBeInTheDocument();
    expect(screen.getByText("500 tested")).toBeInTheDocument();
  });

  it("shows Pause and Cancel buttons when running", () => {
    render(<SearchCard search={makeSearch({ status: "running" })} />);
    expect(screen.getByText("Pause")).toBeInTheDocument();
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("shows Resume and Cancel buttons when paused", () => {
    render(<SearchCard search={makeSearch({ status: "paused" })} />);
    expect(screen.getByText("Resume")).toBeInTheDocument();
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("hides control buttons when completed", () => {
    render(<SearchCard search={makeSearch({ status: "completed" })} />);
    expect(screen.queryByText("Pause")).not.toBeInTheDocument();
    expect(screen.queryByText("Resume")).not.toBeInTheDocument();
    expect(screen.queryByText("Cancel")).not.toBeInTheDocument();
  });

  it("shows Completed status label", () => {
    render(<SearchCard search={makeSearch({ status: "completed" })} />);
    expect(screen.getByText("Completed")).toBeInTheDocument();
  });

  it("shows failed reason", () => {
    const s = makeSearch({
      status: { failed: { reason: "Out of memory" } },
    });
    render(<SearchCard search={s} />);
    expect(screen.getByText("Failed")).toBeInTheDocument();
    expect(screen.getByText("Out of memory")).toBeInTheDocument();
  });
});
