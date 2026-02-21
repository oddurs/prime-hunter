/**
 * @file Tests for the ActivityFeed component
 * @module __tests__/components/activity-feed
 *
 * Validates the global activity feed that displays the most recent prime
 * discoveries on the dashboard. Tests cover empty states, individual prime
 * rendering (expression, form badge, digit count, timestamp), multi-item
 * lists, and the 8-item display cap that prevents the feed from growing
 * unbounded. Data is sourced from the Supabase `primes` table via the
 * `PrimeRecord` type.
 *
 * @see {@link ../../components/activity-feed} Source component
 * @see {@link ../../hooks/use-primes} PrimeRecord type definition
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock format module — provides numberWithCommas, relativeTime, and formLabels
// used by ActivityFeed to render digit counts, timestamps, and form badges.
vi.mock("@/lib/format", () => ({
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  relativeTime: (iso: string) => "5m ago",
  formLabels: {
    factorial: "Factorial",
    kbn: "k\u00b7b^n",
    palindromic: "Palindromic",
    primorial: "Primorial",
  } as Record<string, string>,
}));

import { ActivityFeed } from "@/components/activity-feed";
import type { PrimeRecord } from "@/hooks/use-primes";

/** Factory helper — creates a PrimeRecord with sensible defaults, allowing per-test overrides. */
function makePrime(overrides: Partial<PrimeRecord> = {}): PrimeRecord {
  return {
    id: 1,
    form: "factorial",
    expression: "100!+1",
    digits: 158,
    found_at: "2026-01-15T10:30:00Z",
    proof_method: "deterministic",
    verified: false,
    verified_at: null,
    verification_method: null,
    verification_tier: null,
    ...overrides,
  };
}

// Tests the ActivityFeed component: verifies section title, empty state handling,
// individual prime field rendering (expression, form badge, digit count, timestamp),
// multi-item list rendering, and the 8-item display cap.
describe("ActivityFeed", () => {
  /** Verifies that the feed always shows its section heading regardless of data. */
  it("renders the title 'Recent Discoveries'", () => {
    render(<ActivityFeed primes={[]} />);
    expect(screen.getByText("Recent Discoveries")).toBeInTheDocument();
  });

  /** Verifies the empty state message when the primes array is empty. */
  it("renders empty state when no primes", () => {
    render(<ActivityFeed primes={[]} />);
    expect(screen.getByText("No primes found yet")).toBeInTheDocument();
  });

  /** Verifies the mathematical expression string (e.g. "42!-1") renders in the feed. */
  it("renders prime expression", () => {
    render(<ActivityFeed primes={[makePrime({ expression: "42!-1" })]} />);
    expect(screen.getByText("42!-1")).toBeInTheDocument();
  });

  /** Verifies that known forms display their human-readable label (e.g. "Factorial"). */
  it("renders form label badge", () => {
    render(<ActivityFeed primes={[makePrime({ form: "factorial" })]} />);
    expect(screen.getByText("Factorial")).toBeInTheDocument();
  });

  /** Verifies graceful fallback: unknown form names render as their raw string. */
  it("renders raw form name when no label exists", () => {
    render(
      <ActivityFeed primes={[makePrime({ form: "custom_form" })]} />
    );
    expect(screen.getByText("custom_form")).toBeInTheDocument();
  });

  /** Verifies that digit counts use thousand separators (e.g. "12,345 digits"). */
  it("renders digit count with commas", () => {
    render(<ActivityFeed primes={[makePrime({ digits: 12345 })]} />);
    expect(screen.getByText("12,345 digits")).toBeInTheDocument();
  });

  /** Verifies that the found_at ISO timestamp renders as a human-readable relative time. */
  it("renders relative timestamp", () => {
    render(<ActivityFeed primes={[makePrime()]} />);
    expect(screen.getByText("5m ago")).toBeInTheDocument();
  });

  /** Verifies that multiple primes each render their expression in the list. */
  it("renders multiple primes as list items", () => {
    const primes = [
      makePrime({ id: 1, expression: "5!+1" }),
      makePrime({ id: 2, expression: "7!-1" }),
      makePrime({ id: 3, expression: "11!+1" }),
    ];
    render(<ActivityFeed primes={primes} />);
    expect(screen.getByText("5!+1")).toBeInTheDocument();
    expect(screen.getByText("7!-1")).toBeInTheDocument();
    expect(screen.getByText("11!+1")).toBeInTheDocument();
  });

  /**
   * Verifies the display cap: only the first 8 primes render even if more
   * are provided. This prevents the feed from dominating the dashboard layout.
   */
  it("limits display to 8 primes", () => {
    const primes = Array.from({ length: 12 }, (_, i) =>
      makePrime({ id: i + 1, expression: `${i + 1}!+1` })
    );
    render(<ActivityFeed primes={primes} />);
    // Should show first 8 only
    expect(screen.getByText("1!+1")).toBeInTheDocument();
    expect(screen.getByText("8!+1")).toBeInTheDocument();
    expect(screen.queryByText("9!+1")).not.toBeInTheDocument();
    expect(screen.queryByText("12!+1")).not.toBeInTheDocument();
  });
});
