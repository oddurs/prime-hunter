/**
 * @file Tests for the Releases page
 * @module __tests__/pages/releases
 *
 * Validates the Releases page at `/releases`, which provides canary/ramp/rollback
 * controls for worker binary releases with adoption visibility. The page
 * fetches data from the REST API `/api/releases/*`. Tests verify page heading,
 * subtitle, publish/update release section, rollout controls (Apply, Rollback),
 * empty channel targets, and empty release events.
 *
 * @see {@link ../../app/releases/page} Source page
 * @see {@link ../../app/releases/lib} Release helper functions
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

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

const mockFetchJson = vi.fn();

vi.mock("@/app/releases/lib", () => ({
  DEFAULT_ARTIFACT_JSON: "[]",
  fetchJson: (...args: unknown[]) => mockFetchJson(...args),
  releasesWorkerUrl: () => "http://localhost:3000/api/releases/worker?limit=100",
  releasesEventsUrl: () => "http://localhost:3000/api/releases/events?limit=50",
  releasesHealthUrl: () => "http://localhost:3000/api/releases/health?active_hours=24",
  rolloutBadgeClass: () => "",
  validateArtifacts: () => ({ ok: true, artifacts: [] }),
}));

beforeEach(() => {
  vi.clearAllMocks();
  mockFetchJson.mockImplementation((url: string) => {
    if (url.includes("worker")) {
      return Promise.resolve({ releases: [], channels: [] });
    }
    if (url.includes("events")) {
      return Promise.resolve({ events: [] });
    }
    if (url.includes("health")) {
      return Promise.resolve({ active_hours: 24, adoption: [], channels: [] });
    }
    return Promise.resolve({});
  });
});

import ReleasesPage from "@/app/releases/page";

// Tests the ReleasesPage: heading, subtitle, publish/update section,
// rollout controls, empty channel targets, and empty release events.
describe("ReleasesPage", () => {
  it("renders without crashing", async () => {
    render(<ReleasesPage />);
    await waitFor(() => {
      expect(screen.getByText("Releases")).toBeInTheDocument();
    });
  });

  it("shows subtitle", async () => {
    render(<ReleasesPage />);
    await waitFor(() => {
      expect(
        screen.getByText("Canary/ramp/rollback controls with worker adoption visibility")
      ).toBeInTheDocument();
    });
  });

  it("renders Publish/Update Release section", async () => {
    render(<ReleasesPage />);
    await waitFor(() => {
      expect(screen.getByText("Publish / Update Release")).toBeInTheDocument();
      expect(screen.getByText("Upsert Release Metadata")).toBeInTheDocument();
    });
  });

  it("renders Rollout Controls section", async () => {
    render(<ReleasesPage />);
    await waitFor(() => {
      expect(screen.getByText("Rollout Controls")).toBeInTheDocument();
      expect(screen.getByText("Apply Rollout")).toBeInTheDocument();
      expect(screen.getByText("Rollback Channel")).toBeInTheDocument();
    });
  });

  it("shows empty channel targets", async () => {
    render(<ReleasesPage />);
    await waitFor(() => {
      expect(screen.getByText("No channels configured yet.")).toBeInTheDocument();
    });
  });

  it("shows empty release events", async () => {
    render(<ReleasesPage />);
    await waitFor(() => {
      expect(screen.getByText("No release events recorded.")).toBeInTheDocument();
    });
  });
});
