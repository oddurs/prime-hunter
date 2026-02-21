/**
 * @file Tests for the Logs page
 * @module __tests__/pages/logs
 *
 * Validates the Logs page at `/logs`, which provides a system log viewer
 * with time range selection and component/worker filters. Data is fetched
 * from the REST API `/api/observability/logs`. Tests verify page heading,
 * time range buttons (1h, 6h, 24h, 7d), filter controls (Component,
 * Worker ID), table headers (Time, Level, Component, Message), empty state,
 * and log entry rendering.
 *
 * @see {@link ../../app/logs/page} Source page
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

vi.mock("@/components/view-header", () => ({
  ViewHeader: ({
    title,
    subtitle,
    actions,
  }: {
    title: string;
    subtitle: string;
    actions?: React.ReactNode;
  }) => (
    <div data-testid="view-header">
      <h1>{title}</h1>
      <p>{subtitle}</p>
      {actions}
    </div>
  ),
}));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  formatTime: (t: string) => t,
  relativeTime: (t: string) => t,
}));

const mockFetch = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  global.fetch = mockFetch;
});

import LogsPage from "@/app/logs/page";

// Tests the LogsPage: heading, time range buttons, filter controls,
// table headers, empty state, and log entry rendering.
describe("LogsPage", () => {
  it("renders without crashing", () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ logs: [] }),
    });
    render(<LogsPage />);
    expect(screen.getByText("Logs")).toBeInTheDocument();
  });

  it("shows time range buttons", () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ logs: [] }),
    });
    render(<LogsPage />);
    expect(screen.getByText("1h")).toBeInTheDocument();
    expect(screen.getByText("6h")).toBeInTheDocument();
    expect(screen.getByText("24h")).toBeInTheDocument();
    expect(screen.getByText("7d")).toBeInTheDocument();
  });

  it("renders filter controls", () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ logs: [] }),
    });
    render(<LogsPage />);
    expect(screen.getByText("Filters")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Component")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Worker ID")).toBeInTheDocument();
  });

  it("renders table headers", () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ logs: [] }),
    });
    render(<LogsPage />);
    expect(screen.getByText("Time")).toBeInTheDocument();
    expect(screen.getByText("Level")).toBeInTheDocument();
    expect(screen.getByText("Component")).toBeInTheDocument();
    expect(screen.getByText("Message")).toBeInTheDocument();
  });

  it("shows empty state when no logs", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ logs: [] }),
    });
    render(<LogsPage />);
    await waitFor(() => {
      expect(screen.getByText("No logs in range.")).toBeInTheDocument();
    });
  });

  it("shows logs when data loads", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          logs: [
            {
              id: 1,
              ts: "2026-01-01T00:00:00Z",
              level: "error",
              source: "coordinator",
              component: "fleet",
              message: "Worker heartbeat timeout",
              worker_id: "worker-1",
            },
          ],
        }),
    });
    render(<LogsPage />);
    await waitFor(() => {
      expect(screen.getByText("Worker heartbeat timeout")).toBeInTheDocument();
    });
  });
});
