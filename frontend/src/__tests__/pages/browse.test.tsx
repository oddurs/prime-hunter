/**
 * @file Tests for the Browse page
 * @module __tests__/pages/browse
 *
 * Validates the Browse page at `/browse`, which provides a filterable,
 * paginated archive of all discovered primes. The page consumes usePrimes
 * and useStats hooks via Supabase. Tests verify page heading, subtitle
 * with total prime count, filter controls (expression search, min/max
 * digits), table rendering with prime data, pagination controls, and
 * column headers.
 *
 * @see {@link ../../app/browse/page} Source page
 * @see {@link ../../hooks/use-primes} usePrimes hook (data provider)
 * @see {@link ../../hooks/use-stats} useStats hook (total count)
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock hooks used by BrowsePage

const mockFetchPrimes = vi.fn();
const mockFetchPrimeDetail = vi.fn();
const mockClearSelectedPrime = vi.fn();

vi.mock("@/hooks/use-primes", () => ({
  usePrimes: () => ({
    primes: {
      primes: [
        {
          id: 1,
          form: "factorial",
          expression: "5!+1",
          digits: 3,
          found_at: "2026-01-01T00:00:00Z",
          proof_method: "deterministic",
          verified: true,
          verified_at: null,
          verification_method: null,
          verification_tier: null,
        },
        {
          id: 2,
          form: "kbn",
          expression: "3*2^10+1",
          digits: 4,
          found_at: "2026-01-02T00:00:00Z",
          proof_method: "probabilistic",
          verified: false,
          verified_at: null,
          verification_method: null,
          verification_tier: null,
        },
      ],
      total: 2,
      offset: 0,
      limit: 50,
    },
    fetchPrimes: mockFetchPrimes,
    fetchPrimeDetail: mockFetchPrimeDetail,
    selectedPrime: null,
    clearSelectedPrime: mockClearSelectedPrime,
  }),
}));

vi.mock("@/hooks/use-stats", () => ({
  useStats: () => ({
    stats: {
      total: 42,
      by_form: [
        { form: "factorial", count: 30 },
        { form: "kbn", count: 12 },
      ],
      largest_digits: 1000,
      largest_expression: "100!+1",
    },
  }),
}));

vi.mock("@/components/view-header", () => ({
  ViewHeader: ({
    title,
    subtitle,
  }: {
    title: string;
    subtitle: string;
  }) => (
    <div data-testid="view-header">
      <h1>{title}</h1>
      <p>{subtitle}</p>
    </div>
  ),
}));

vi.mock("@/components/prime-detail-dialog", () => ({
  PrimeDetailDialog: () => null,
}));

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
  }: {
    children: React.ReactNode;
    href: string;
  }) => <a href={href}>{children}</a>,
}));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  formatTime: (t: string) => t,
  formToSlug: (f: string) => f.toLowerCase(),
}));

import BrowsePage from "@/app/browse/page";

// Tests the BrowsePage: heading, subtitle with count, filter controls,
// prime expression rendering, pagination, and column headers.
describe("BrowsePage", () => {
  it("renders without crashing", () => {
    render(<BrowsePage />);
    expect(screen.getByText("Browse")).toBeInTheDocument();
  });

  it("shows total prime count in subtitle", () => {
    render(<BrowsePage />);
    expect(screen.getByText("2 primes in the archive")).toBeInTheDocument();
  });

  it("renders filter controls", () => {
    render(<BrowsePage />);
    expect(screen.getByPlaceholderText("Expression contains...")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("e.g. 100")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("e.g. 2000")).toBeInTheDocument();
  });

  it("renders table with prime expressions", () => {
    render(<BrowsePage />);
    expect(screen.getByText("5!+1")).toBeInTheDocument();
    expect(screen.getByText("3*2^10+1")).toBeInTheDocument();
  });

  it("renders pagination controls", () => {
    render(<BrowsePage />);
    expect(screen.getByText("Previous")).toBeInTheDocument();
    expect(screen.getByText("Next")).toBeInTheDocument();
  });

  it("renders column headers", () => {
    render(<BrowsePage />);
    expect(screen.getByText("Expression")).toBeInTheDocument();
    expect(screen.getByText("Digits")).toBeInTheDocument();
    // "Form" and "Found" appear in both filters and table, so just check they exist
    expect(screen.getAllByText("Form").length).toBeGreaterThanOrEqual(1);
  });
});
