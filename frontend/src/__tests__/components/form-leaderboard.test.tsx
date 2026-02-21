/**
 * @file Tests for the FormLeaderboard ranking table
 * @module __tests__/components/form-leaderboard
 *
 * Validates the form leaderboard table that ranks prime forms by count,
 * largest discovery, verification percentage, and recency. Tests cover
 * table rendering with form labels, column headers, sortable columns
 * (count, largest digits), verified percentage color coding (green >= 90%,
 * yellow >= 50%, red < 50%), relative timestamps, unknown form fallback,
 * and empty data handling.
 *
 * @see {@link ../../components/form-leaderboard} Source component
 * @see {@link ../../hooks/use-form-leaderboard} Data hook and FormLeaderboardEntry type
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FormLeaderboard } from "@/components/form-leaderboard";
import type { FormLeaderboardEntry } from "@/hooks/use-form-leaderboard";

vi.mock("@/lib/format", () => ({
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  relativeTime: (iso: string) => {
    // Return a predictable string for testing
    if (iso.includes("2026-02-20")) return "just now";
    return "2h 15m ago";
  },
  formLabels: {
    factorial: "Factorial",
    kbn: "k\u00b7b^n",
    palindromic: "Palindromic",
    twin: "Twin",
  } as Record<string, string>,
}));

/** Factory helper — creates a FormLeaderboardEntry with sensible defaults. */
function makeEntry(overrides: Partial<FormLeaderboardEntry> = {}): FormLeaderboardEntry {
  return {
    form: "factorial",
    count: 150,
    largest_digits: 45678,
    largest_expression: "100000!+1",
    latest_found_at: "2026-02-15T00:00:00Z",
    verified_count: 140,
    verified_pct: 93,
    ...overrides,
  };
}

// Tests the FormLeaderboard table: form label rendering, column values,
// verified percentage color coding, sorting behavior, and empty data handling.
describe("FormLeaderboard", () => {
  /** Verifies the section title and that form entries display their human-readable labels. */
  it("renders table with form names", () => {
    const entries = [
      makeEntry({ form: "factorial" }),
      makeEntry({ form: "kbn", count: 80 }),
    ];

    render(<FormLeaderboard entries={entries} />);

    expect(screen.getByText("Form Leaderboard")).toBeInTheDocument();
    expect(screen.getByText("Factorial")).toBeInTheDocument();
    expect(screen.getByText("k\u00b7b^n")).toBeInTheDocument();
  });

  /** Verifies count and largest digit values render with proper formatting. */
  it("shows count and largest digits columns", () => {
    const entries = [makeEntry({ count: 150, largest_digits: 45678 })];

    render(<FormLeaderboard entries={entries} />);

    expect(screen.getByText("150")).toBeInTheDocument();
    expect(screen.getByText("45,678 digits")).toBeInTheDocument();
  });

  /** Verifies the verified percentage renders with the % suffix. */
  it("shows verified percentage", () => {
    const entries = [makeEntry({ verified_pct: 93 })];

    render(<FormLeaderboard entries={entries} />);

    expect(screen.getByText("93%")).toBeInTheDocument();
  });

  /** Verifies the "Last Found" column uses relativeTime formatting. */
  it("shows relative time for last found", () => {
    const entries = [makeEntry({ latest_found_at: "2026-02-15T00:00:00Z" })];

    render(<FormLeaderboard entries={entries} />);

    expect(screen.getByText("2h 15m ago")).toBeInTheDocument();
  });

  /** Verifies the component renders nothing when the entries array is empty. */
  it("returns null for empty data", () => {
    const { container } = render(<FormLeaderboard entries={[]} />);
    expect(container.innerHTML).toBe("");
  });

  /** Verifies known forms display their mapped label (e.g. "Palindromic"). */
  it("uses form label for known forms", () => {
    const entries = [makeEntry({ form: "palindromic" })];

    render(<FormLeaderboard entries={entries} />);

    expect(screen.getByText("Palindromic")).toBeInTheDocument();
  });

  /** Verifies unknown forms fall back to displaying the raw form identifier. */
  it("falls back to raw form name for unknown forms", () => {
    const entries = [makeEntry({ form: "exotic_form" })];

    render(<FormLeaderboard entries={entries} />);

    expect(screen.getByText("exotic_form")).toBeInTheDocument();
  });

  /** Verifies green text-color class for verified percentage >= 90%. */
  it("applies green color for verified >= 90%", () => {
    const entries = [makeEntry({ verified_pct: 95 })];

    render(<FormLeaderboard entries={entries} />);

    const pctElement = screen.getByText("95%");
    expect(pctElement.className).toContain("text-green-500");
  });

  /** Verifies yellow text-color class for verified percentage between 50% and 89%. */
  it("applies yellow color for verified >= 50% and < 90%", () => {
    const entries = [makeEntry({ verified_pct: 60 })];

    render(<FormLeaderboard entries={entries} />);

    const pctElement = screen.getByText("60%");
    expect(pctElement.className).toContain("text-yellow-500");
  });

  /** Verifies red text-color class for verified percentage below 50%. */
  it("applies red color for verified < 50%", () => {
    const entries = [makeEntry({ verified_pct: 30 })];

    render(<FormLeaderboard entries={entries} />);

    const pctElement = screen.getByText("30%");
    expect(pctElement.className).toContain("text-red-500");
  });

  /** Verifies all five column headers render: Form, Count, Largest, Last Found, Verified. */
  it("renders column headers", () => {
    const entries = [makeEntry()];

    render(<FormLeaderboard entries={entries} />);

    expect(screen.getByText("Form")).toBeInTheDocument();
    // Count header includes sort indicator
    expect(screen.getByText(/Count/)).toBeInTheDocument();
    expect(screen.getByText(/Largest/)).toBeInTheDocument();
    expect(screen.getByText(/Last Found/)).toBeInTheDocument();
    expect(screen.getByText(/Verified/)).toBeInTheDocument();
  });

  /** Verifies clicking a sortable column header toggles between desc and asc order. */
  it("toggles sort direction on column click", async () => {
    const user = userEvent.setup();
    const entries = [
      makeEntry({ form: "factorial", count: 150 }),
      makeEntry({ form: "kbn", count: 80 }),
    ];

    render(<FormLeaderboard entries={entries} />);

    // Default sort is count desc, so factorial (150) should be first
    const rows = screen.getAllByRole("row");
    // rows[0] is the header, rows[1] is first data row
    expect(rows[1]).toHaveTextContent("Factorial");
    expect(rows[2]).toHaveTextContent("k\u00b7b^n");

    // Click Count header to toggle to ascending
    const countHeader = screen.getByText(/Count/);
    await user.click(countHeader);

    // Now kbn (80) should be first
    const rowsAfter = screen.getAllByRole("row");
    expect(rowsAfter[1]).toHaveTextContent("k\u00b7b^n");
    expect(rowsAfter[2]).toHaveTextContent("Factorial");
  });

  /** Verifies sorting by a different column (Largest) re-orders rows correctly. */
  it("sorts by largest digits when clicking that column", async () => {
    const user = userEvent.setup();
    const entries = [
      makeEntry({ form: "factorial", largest_digits: 45678 }),
      makeEntry({ form: "kbn", largest_digits: 99999 }),
    ];

    render(<FormLeaderboard entries={entries} />);

    // Click Largest header
    const largestHeader = screen.getByText(/Largest/);
    await user.click(largestHeader);

    // Default new sort direction is desc, kbn (99999) should be first
    const rows = screen.getAllByRole("row");
    expect(rows[1]).toHaveTextContent("k\u00b7b^n");
    expect(rows[2]).toHaveTextContent("Factorial");
  });
});
