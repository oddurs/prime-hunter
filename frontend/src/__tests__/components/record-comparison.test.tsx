/**
 * @file Tests for the RecordComparison component
 * @module __tests__/components/record-comparison
 *
 * Validates the world record comparison card that displays how our best
 * discovery for a given prime form compares to the known world record.
 * Tests cover form label rendering, record digit count, record holder name,
 * our best digits, "none" fallback, percentage progress, expression
 * truncation, gap analysis, t5k.org external links, and estimated
 * compute effort for record-sized candidates.
 *
 * @see {@link ../../components/record-comparison} Source component
 * @see {@link ../../hooks/use-records} World record data hook
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock lucide-react icons used in the comparison card.
vi.mock("lucide-react", () => ({
  ExternalLink: () => <span data-testid="external-link-icon" />,
}));

// Mock format module
vi.mock("@/lib/format", () => ({
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  formLabels: {
    factorial: "Factorial",
    kbn: "k\u00b7b^n",
    palindromic: "Palindromic",
    primorial: "Primorial",
    wagstaff: "Wagstaff",
    twin: "Twin",
  } as Record<string, string>,
}));

import { RecordComparison } from "@/components/record-comparison";

interface RecordSummary {
  form: string;
  expression: string;
  digits: number;
  holder: string | null;
  our_best_digits: number;
}

function makeRecord(overrides: Partial<RecordSummary> = {}): RecordSummary {
  return {
    form: "factorial",
    expression: "208003!+1",
    digits: 1015843,
    holder: "Caldwell",
    our_best_digits: 50000,
    ...overrides,
  };
}

// Tests the RecordComparison card: form label, record digits, holder name,
// our best digits, percentage progress, expression truncation, gap analysis,
// t5k.org links, and compute effort estimates.
describe("RecordComparison", () => {
  it("renders form label", () => {
    render(<RecordComparison record={makeRecord()} />);
    expect(screen.getByText("Factorial")).toBeInTheDocument();
  });

  it("renders raw form name when no label exists", () => {
    render(
      <RecordComparison record={makeRecord({ form: "custom_form" })} />
    );
    expect(screen.getByText("custom_form")).toBeInTheDocument();
  });

  it("renders record digit count with commas", () => {
    render(
      <RecordComparison record={makeRecord({ digits: 1015843 })} />
    );
    expect(screen.getByText("1,015,843 digits")).toBeInTheDocument();
  });

  it("renders holder name", () => {
    render(
      <RecordComparison record={makeRecord({ holder: "Caldwell" })} />
    );
    expect(screen.getByText(/Caldwell/)).toBeInTheDocument();
  });

  it("renders 'unknown' when holder is null", () => {
    render(
      <RecordComparison record={makeRecord({ holder: null })} />
    );
    expect(screen.getByText(/unknown/)).toBeInTheDocument();
  });

  it("renders our best digits with commas", () => {
    render(
      <RecordComparison
        record={makeRecord({ our_best_digits: 50000 })}
      />
    );
    expect(screen.getByText(/50,000/)).toBeInTheDocument();
  });

  it("shows 'none' when our_best_digits is 0", () => {
    render(
      <RecordComparison
        record={makeRecord({ our_best_digits: 0 })}
      />
    );
    expect(screen.getByText(/none/)).toBeInTheDocument();
  });

  it("renders percentage progress", () => {
    render(
      <RecordComparison
        record={makeRecord({ digits: 100000, our_best_digits: 50000 })}
      />
    );
    expect(screen.getByText("50.0%")).toBeInTheDocument();
  });

  it("truncates long expressions", () => {
    const longExpr = "123456789012345678901234567890";
    render(
      <RecordComparison
        record={makeRecord({ expression: longExpr })}
      />
    );
    // Should be truncated to 25 chars + "..."
    expect(screen.getByText(/\.\.\./)).toBeInTheDocument();
  });

  it("does not truncate short expressions", () => {
    render(
      <RecordComparison
        record={makeRecord({ expression: "5!+1" })}
      />
    );
    expect(screen.getByText(/5!\+1/)).toBeInTheDocument();
  });

  it("renders gap analysis", () => {
    render(
      <RecordComparison
        record={makeRecord({
          digits: 100000,
          our_best_digits: 40000,
        })}
      />
    );
    expect(screen.getByText(/60,000/)).toBeInTheDocument();
    expect(screen.getByText(/digits to record/)).toBeInTheDocument();
  });

  it("renders t5k.org link for known forms", () => {
    render(<RecordComparison record={makeRecord({ form: "factorial" })} />);
    const link = screen.getByText("t5k.org Top 20");
    expect(link).toBeInTheDocument();
    expect(link.closest("a")?.getAttribute("href")).toContain("t5k.org");
  });

  it("does not render t5k.org link for unknown forms", () => {
    render(
      <RecordComparison record={makeRecord({ form: "custom_form" })} />
    );
    expect(screen.queryByText("t5k.org Top 20")).not.toBeInTheDocument();
  });

  it("renders core-year estimate for large records", () => {
    render(
      <RecordComparison
        record={makeRecord({ digits: 100000 })}
      />
    );
    // Should show some estimate (core-hrs or core-yrs)
    expect(
      screen.getByText(/to find one at record size/)
    ).toBeInTheDocument();
  });
});
