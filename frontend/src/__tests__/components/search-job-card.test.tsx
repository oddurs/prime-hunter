/**
 * @file Tests for the SearchJobCard component
 * @module __tests__/components/search-job-card
 *
 * Validates the search job card that represents a distributed search job
 * with block-based work distribution. Tests cover form label badge, status
 * labels (Running/Paused/Completed), job ID display, tested/found counts,
 * block count calculation from range and block_size, action buttons per
 * status, error message display, ended timestamp, range display, and
 * unknown form fallback.
 *
 * @see {@link ../../components/search-job-card} Source component
 * @see {@link ../../hooks/use-websocket} SearchJob type
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock lucide-react icons used for action buttons.
vi.mock("lucide-react", () => ({
  Pause: () => <span data-testid="pause-icon" />,
  Play: () => <span data-testid="play-icon" />,
  X: () => <span data-testid="x-icon" />,
}));

// Mock format module
vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  formLabels: {
    factorial: "Factorial",
    kbn: "k\u00b7b^n",
    palindromic: "Palindromic",
  } as Record<string, string>,
  relativeTime: (iso: string) => "2h ago",
}));

// Mock fetch
vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true }));

import { SearchJobCard } from "@/components/search-job-card";
import type { SearchJob } from "@/hooks/use-websocket";

function makeJob(overrides: Partial<SearchJob> = {}): SearchJob {
  return {
    id: 1,
    search_type: "factorial",
    params: { start: 1000, end: 5000 },
    status: "running",
    error: null,
    created_at: "2026-01-15T10:00:00Z",
    started_at: "2026-01-15T10:00:00Z",
    stopped_at: null,
    range_start: 1000,
    range_end: 5000,
    block_size: 100,
    total_tested: 2500,
    total_found: 3,
    ...overrides,
  };
}

// Tests the SearchJobCard: form badge, status labels, job ID, tested/found stats,
// block count, action buttons, error messages, timestamps, range info, and fallbacks.
describe("SearchJobCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders form label badge", () => {
    render(<SearchJobCard job={makeJob()} />);
    expect(screen.getByText("Factorial")).toBeInTheDocument();
  });

  it("renders status label for running job", () => {
    render(<SearchJobCard job={makeJob({ status: "running" })} />);
    expect(screen.getByText("Running")).toBeInTheDocument();
  });

  it("renders status label for paused job", () => {
    render(<SearchJobCard job={makeJob({ status: "paused" })} />);
    expect(screen.getByText("Paused")).toBeInTheDocument();
  });

  it("renders status label for completed job", () => {
    render(<SearchJobCard job={makeJob({ status: "completed" })} />);
    expect(screen.getByText("Completed")).toBeInTheDocument();
  });

  it("renders job ID", () => {
    render(<SearchJobCard job={makeJob({ id: 42 })} />);
    expect(screen.getByText("Job #42")).toBeInTheDocument();
  });

  it("renders tested count with commas", () => {
    render(<SearchJobCard job={makeJob({ total_tested: 12345 })} />);
    expect(screen.getByText("12,345 tested")).toBeInTheDocument();
  });

  it("renders found count", () => {
    render(<SearchJobCard job={makeJob({ total_found: 7 })} />);
    expect(screen.getByText("7 found")).toBeInTheDocument();
  });

  it("renders block count", () => {
    render(
      <SearchJobCard
        job={makeJob({
          range_start: 0,
          range_end: 1000,
          block_size: 100,
        })}
      />
    );
    expect(screen.getByText("10 blocks")).toBeInTheDocument();
  });

  it("shows Pause button for running job", () => {
    render(<SearchJobCard job={makeJob({ status: "running" })} />);
    expect(screen.getByText("Pause")).toBeInTheDocument();
  });

  it("shows Resume button for paused job", () => {
    render(<SearchJobCard job={makeJob({ status: "paused" })} />);
    expect(screen.getByText("Resume")).toBeInTheDocument();
  });

  it("shows Cancel button for running job", () => {
    render(<SearchJobCard job={makeJob({ status: "running" })} />);
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("shows Cancel button for paused job", () => {
    render(<SearchJobCard job={makeJob({ status: "paused" })} />);
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("hides action buttons for completed job", () => {
    render(<SearchJobCard job={makeJob({ status: "completed" })} />);
    expect(screen.queryByText("Pause")).not.toBeInTheDocument();
    expect(screen.queryByText("Resume")).not.toBeInTheDocument();
    expect(screen.queryByText("Cancel")).not.toBeInTheDocument();
  });

  it("hides action buttons for cancelled job", () => {
    render(<SearchJobCard job={makeJob({ status: "cancelled" })} />);
    expect(screen.queryByText("Pause")).not.toBeInTheDocument();
    expect(screen.queryByText("Resume")).not.toBeInTheDocument();
    expect(screen.queryByText("Cancel")).not.toBeInTheDocument();
  });

  it("displays error message when present", () => {
    render(
      <SearchJobCard
        job={makeJob({
          status: "failed",
          error: "Out of memory",
        })}
      />
    );
    expect(screen.getByText("Out of memory")).toBeInTheDocument();
  });

  it("does not display error section when error is null", () => {
    render(<SearchJobCard job={makeJob({ error: null })} />);
    expect(screen.queryByText("Out of memory")).not.toBeInTheDocument();
  });

  it("shows ended time when stopped_at is present", () => {
    render(
      <SearchJobCard
        job={makeJob({
          status: "completed",
          stopped_at: "2026-01-16T00:00:00Z",
        })}
      />
    );
    expect(screen.getByText(/ended/)).toBeInTheDocument();
  });

  it("renders range info", () => {
    render(
      <SearchJobCard
        job={makeJob({ range_start: 1000, range_end: 5000 })}
      />
    );
    expect(screen.getByText("range 1,000..5,000")).toBeInTheDocument();
  });

  it("renders search type as fallback when no label exists", () => {
    render(
      <SearchJobCard
        job={makeJob({ search_type: "unknown_form" })}
      />
    );
    expect(screen.getByText("unknown_form")).toBeInTheDocument();
  });
});
