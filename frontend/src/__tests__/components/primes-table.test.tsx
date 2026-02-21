/**
 * @file Tests for the PrimesTable component
 * @module __tests__/components/primes-table
 *
 * Validates the prime number results table — the primary data display component
 * of the dashboard. Tests cover rendering, pagination, sorting, filtering,
 * and empty states. The table displays prime records from the Supabase `primes`
 * table including expression, form, digit count, and discovery timestamp.
 * It integrates with the usePrimes hook for data fetching and filtering.
 *
 * @see {@link ../../components/primes-table} Source component
 * @see {@link ../../hooks/use-primes} usePrimes hook (data provider)
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock next/link to render a plain <a> for testing.
vi.mock("next/link", () => ({
  default: ({
    children,
    href,
  }: {
    children: React.ReactNode;
    href: string;
  }) => <a href={href}>{children}</a>,
}));

// Mock format module
vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  formToSlug: (form: string) => form.toLowerCase(),
}));

// Mock usePrimes hook
const mockFetchPrimes = vi.fn();
const mockFetchPrimeDetail = vi.fn();
const mockClearSelectedPrime = vi.fn();

let mockPrimesData = {
  primes: [] as Array<{
    id: number;
    form: string;
    expression: string;
    digits: number;
    found_at: string;
  }>,
  total: 0,
  limit: 50,
  offset: 0,
};

vi.mock("@/hooks/use-primes", () => ({
  usePrimes: () => ({
    primes: mockPrimesData,
    selectedPrime: null,
    fetchPrimes: mockFetchPrimes,
    fetchPrimeDetail: mockFetchPrimeDetail,
    clearSelectedPrime: mockClearSelectedPrime,
  }),
}));

// Mock PrimeDetailDialog
vi.mock("@/components/prime-detail-dialog", () => ({
  PrimeDetailDialog: ({ open }: { open: boolean }) =>
    open ? <div data-testid="prime-detail-dialog">Dialog</div> : null,
}));

import { PrimesTable } from "@/components/primes-table";

const samplePrimes = [
  {
    id: 1,
    form: "factorial",
    expression: "5!+1",
    digits: 3,
    found_at: "2026-01-15T10:30:00Z",
  },
  {
    id: 2,
    form: "kbn",
    expression: "3*2^100+1",
    digits: 31,
    found_at: "2026-01-16T14:00:00Z",
  },
  {
    id: 3,
    form: "palindromic",
    expression: "12321",
    digits: 5,
    found_at: "2026-01-17T08:00:00Z",
  },
];

const sampleStats = {
  total: 100,
  by_form: [
    { form: "factorial", count: 40 },
    { form: "kbn", count: 35 },
    { form: "palindromic", count: 25 },
  ],
  largest_digits: 45678,
  largest_expression: "99999!+1",
};

// Tests the PrimesTable: empty state, prime data rendering, column headers,
// pagination info, disabled prev/next buttons, form badges, digit formatting,
// title, search input, export button, and null stats handling.
describe("PrimesTable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPrimesData = {
      primes: [],
      total: 0,
      limit: 50,
      offset: 0,
    };
  });

  it("renders empty state when no primes", () => {
    render(<PrimesTable stats={sampleStats} />);
    expect(screen.getByText("No primes found yet")).toBeInTheDocument();
  });

  it("renders table with prime data", () => {
    mockPrimesData = {
      primes: samplePrimes,
      total: 3,
      limit: 50,
      offset: 0,
    };
    render(<PrimesTable stats={sampleStats} />);
    expect(screen.getByText("5!+1")).toBeInTheDocument();
    expect(screen.getByText("3*2^100+1")).toBeInTheDocument();
    expect(screen.getByText("12321")).toBeInTheDocument();
  });

  it("renders column headers", () => {
    render(<PrimesTable stats={sampleStats} />);
    // Headers contain sort indicator characters
    expect(screen.getByText(/Expression/)).toBeInTheDocument();
    expect(screen.getByText(/Form/)).toBeInTheDocument();
    expect(screen.getByText(/Digits/)).toBeInTheDocument();
    expect(screen.getByText(/Found/)).toBeInTheDocument();
  });

  it("renders pagination info", () => {
    mockPrimesData = {
      primes: samplePrimes,
      total: 3,
      limit: 50,
      offset: 0,
    };
    render(<PrimesTable stats={sampleStats} />);
    // Pagination text appears in both header and footer
    const matches = screen.getAllByText("1-3 of 3");
    expect(matches.length).toBe(2);
  });

  it("renders 0 results when total is zero", () => {
    render(<PrimesTable stats={sampleStats} />);
    expect(screen.getByText("0 results")).toBeInTheDocument();
  });

  it("disables Previous button on first page", () => {
    mockPrimesData = {
      primes: samplePrimes,
      total: 3,
      limit: 50,
      offset: 0,
    };
    render(<PrimesTable stats={sampleStats} />);
    const prevButton = screen.getByText("Previous");
    expect(prevButton).toBeDisabled();
  });

  it("disables Next button when on last page", () => {
    mockPrimesData = {
      primes: samplePrimes,
      total: 3,
      limit: 50,
      offset: 0,
    };
    render(<PrimesTable stats={sampleStats} />);
    const nextButton = screen.getByText("Next");
    expect(nextButton).toBeDisabled();
  });

  it("renders form badges for each prime", () => {
    mockPrimesData = {
      primes: samplePrimes,
      total: 3,
      limit: 50,
      offset: 0,
    };
    render(<PrimesTable stats={sampleStats} />);
    expect(screen.getByText("factorial")).toBeInTheDocument();
    expect(screen.getByText("kbn")).toBeInTheDocument();
    expect(screen.getByText("palindromic")).toBeInTheDocument();
  });

  it("renders digit count with commas", () => {
    mockPrimesData = {
      primes: [
        {
          id: 1,
          form: "factorial",
          expression: "large!+1",
          digits: 12345,
          found_at: "2026-01-15T00:00:00Z",
        },
      ],
      total: 1,
      limit: 50,
      offset: 0,
    };
    render(<PrimesTable stats={sampleStats} />);
    expect(screen.getByText("12,345")).toBeInTheDocument();
  });

  it("shows title 'Recent primes' when no filters active", () => {
    render(<PrimesTable stats={sampleStats} />);
    expect(screen.getByText("Recent primes")).toBeInTheDocument();
  });

  it("renders search input", () => {
    render(<PrimesTable stats={sampleStats} />);
    expect(
      screen.getByPlaceholderText("Search expressions...")
    ).toBeInTheDocument();
  });

  it("renders Export button", () => {
    render(<PrimesTable stats={sampleStats} />);
    expect(screen.getByText("Export")).toBeInTheDocument();
  });

  it("handles null stats gracefully", () => {
    render(<PrimesTable stats={null} />);
    expect(screen.getByText("Recent primes")).toBeInTheDocument();
  });
});
