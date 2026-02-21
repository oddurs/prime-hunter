/**
 * @file Tests for the Leaderboard page
 * @module __tests__/pages/leaderboard
 *
 * Validates the Leaderboard page at `/leaderboard`, which displays volunteer
 * contributor rankings. Data is fetched from the REST API. Tests verify page
 * heading, loading state, empty state, leaderboard entries with usernames
 * and teams, error handling for failed fetches, and fleet stats banner cards
 * (Volunteers, Primes Found, Total Compute, Active Workers).
 *
 * @see {@link ../../app/leaderboard/page} Source page
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

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

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) => String(x),
}));

const mockFetch = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  global.fetch = mockFetch;
});

import LeaderboardPage from "@/app/leaderboard/page";

// Tests the LeaderboardPage: heading, loading state, empty state, entries
// with usernames/teams, fetch error handling, and fleet stats banner.
describe("LeaderboardPage", () => {
  it("renders without crashing", () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });
    render(<LeaderboardPage />);
    expect(screen.getByText("Leaderboard")).toBeInTheDocument();
  });

  it("shows loading state initially", () => {
    mockFetch.mockReturnValue(new Promise(() => {}));
    render(<LeaderboardPage />);
    expect(screen.getByText("Loading leaderboard...")).toBeInTheDocument();
  });

  it("shows empty state when no entries", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });
    render(<LeaderboardPage />);
    await waitFor(() => {
      expect(screen.getByText("No volunteers yet. Be the first!")).toBeInTheDocument();
    });
  });

  it("shows leaderboard entries when data loads", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve([
          {
            rank: 1,
            username: "alice",
            team: "TeamAlpha",
            credit: 100000,
            primes_found: 5,
            worker_count: 2,
          },
          {
            rank: 2,
            username: "bob",
            team: null,
            credit: 50000,
            primes_found: 0,
            worker_count: 1,
          },
        ]),
    });
    render(<LeaderboardPage />);
    await waitFor(() => {
      expect(screen.getByText("alice")).toBeInTheDocument();
      expect(screen.getByText("bob")).toBeInTheDocument();
      expect(screen.getByText("TeamAlpha")).toBeInTheDocument();
    });
  });

  it("shows error state on fetch failure", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });
    render(<LeaderboardPage />);
    await waitFor(() => {
      expect(screen.getByText("HTTP 500")).toBeInTheDocument();
    });
  });

  it("renders fleet stats banner cards", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });
    render(<LeaderboardPage />);
    await waitFor(() => {
      expect(screen.getByText("Volunteers")).toBeInTheDocument();
      expect(screen.getByText("Primes Found")).toBeInTheDocument();
      expect(screen.getByText("Total Compute")).toBeInTheDocument();
      expect(screen.getByText("Active Workers")).toBeInTheDocument();
    });
  });
});
