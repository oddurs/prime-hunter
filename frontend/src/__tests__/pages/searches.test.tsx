/**
 * @file Tests for the Searches page
 * @module __tests__/pages/searches
 *
 * Validates the Searches page at `/searches`, which provides search process
 * management with running/completed/failed status tracking. The page consumes
 * WebSocket context for real-time search and job data. Tests verify page
 * heading, subtitle with running count, stat cards (Running, Completed,
 * Failed), "New Search" button, empty state, and tab navigation (All,
 * Running, Jobs).
 *
 * @see {@link ../../app/searches/page} Source page
 * @see {@link ../../hooks/use-websocket} ManagedSearch, SearchJob types
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("@/contexts/websocket-context", () => ({
  useWs: () => ({
    status: { active: false, checkpoint: null },
    fleet: {
      workers: [],
      total_workers: 0,
      total_cores: 0,
      total_tested: 0,
      total_found: 0,
    },
    searches: [],
    searchJobs: [],
    connected: true,
  }),
}));

vi.mock("@/components/view-header", () => ({
  ViewHeader: ({
    title,
    subtitle,
    actions,
    tabs,
  }: {
    title: string;
    subtitle: string;
    actions?: React.ReactNode;
    tabs?: React.ReactNode;
  }) => (
    <div data-testid="view-header">
      <h1>{title}</h1>
      <p>{subtitle}</p>
      {actions}
      {tabs}
    </div>
  ),
}));

vi.mock("@/components/search-card", () => ({
  SearchCard: ({ search }: { search: { id: string } }) => (
    <div data-testid="search-card">Search #{search.id}</div>
  ),
}));

vi.mock("@/components/search-job-card", () => ({
  SearchJobCard: () => <div data-testid="search-job-card">Job</div>,
}));

vi.mock("@/components/new-search-dialog", () => ({
  NewSearchDialog: () => null,
}));

vi.mock("@/components/stat-card", () => ({
  StatCard: ({ label, value }: { label: string; value: React.ReactNode }) => (
    <div data-testid={`stat-${label.toLowerCase().replace(/\s+/g, "-")}`}>
      {label}: {value}
    </div>
  ),
}));

vi.mock("@/components/empty-state", () => ({
  EmptyState: ({ message }: { message: string }) => (
    <div data-testid="empty-state">{message}</div>
  ),
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

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) => String(x),
}));

import SearchesPage from "@/app/searches/page";

// Tests the SearchesPage: heading, subtitle, stat cards, New Search button,
// empty state, and tab navigation (All, Running, Jobs).
describe("SearchesPage", () => {
  it("renders without crashing", () => {
    render(<SearchesPage />);
    expect(screen.getByText("Searches")).toBeInTheDocument();
  });

  it("shows 0 running searches in subtitle", () => {
    render(<SearchesPage />);
    expect(screen.getByText(/0 running/)).toBeInTheDocument();
  });

  it("renders stat cards", () => {
    render(<SearchesPage />);
    expect(screen.getByTestId("stat-running")).toBeInTheDocument();
    expect(screen.getByTestId("stat-completed")).toBeInTheDocument();
    expect(screen.getByTestId("stat-failed")).toBeInTheDocument();
  });

  it("shows New Search button", () => {
    render(<SearchesPage />);
    expect(screen.getByText("New Search")).toBeInTheDocument();
  });

  it("shows empty state when no searches", () => {
    render(<SearchesPage />);
    const emptyStates = screen.getAllByTestId("empty-state");
    expect(emptyStates.length).toBeGreaterThanOrEqual(1);
  });

  it("renders tabs for All, Running, Jobs", () => {
    render(<SearchesPage />);
    expect(screen.getByText("All")).toBeInTheDocument();
    expect(screen.getByText("Running")).toBeInTheDocument();
    expect(screen.getByText("Jobs")).toBeInTheDocument();
  });
});
