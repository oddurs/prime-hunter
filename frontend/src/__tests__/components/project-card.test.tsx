/**
 * @file Tests for the ProjectCard component
 * @module __tests__/components/project-card
 *
 * Validates the project campaign card shown on the Projects page. Each card
 * represents a prime discovery campaign with a name, form, objective, status,
 * and aggregate statistics. Tests cover name/status/form/objective rendering,
 * tested/found counts, best digit display, cost display, status-dependent
 * action buttons (Activate/Resume/Pause), detail page links, and optional
 * selection checkbox for bulk operations.
 *
 * @see {@link ../../components/project-card} Source component
 * @see {@link ../../hooks/use-projects} Project data types
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ProjectCard } from "@/components/project-card";

// Mock next/link to render a plain anchor for link testing.
vi.mock("next/link", () => ({
  default: ({
    children,
    href,
  }: {
    children: React.ReactNode;
    href: string;
  }) => <a href={href}>{children}</a>,
}));

// Mock sonner toast
vi.mock("sonner", () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

// Mock fetch for action buttons
vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true }));

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
}));

interface ProjectSummary {
  slug: string;
  name: string;
  form: string;
  objective: string;
  status: string;
  total_tested: number;
  total_found: number;
  best_digits: number;
  total_cost_usd: number;
}

function makeProject(overrides: Partial<ProjectSummary> = {}): ProjectSummary {
  return {
    slug: "factorial-million",
    name: "Factorial Million",
    form: "factorial",
    objective: "record",
    status: "active",
    total_tested: 50000,
    total_found: 12,
    best_digits: 45678,
    total_cost_usd: 3.25,
    ...overrides,
  };
}

// Tests the ProjectCard: name, status badge, form, objective, tested/found counts,
// best digits, cost, action buttons per status, detail page link, and selection checkbox.
describe("ProjectCard", () => {
  it("renders project name", () => {
    render(<ProjectCard project={makeProject()} />);
    expect(screen.getByText("Factorial Million")).toBeInTheDocument();
  });

  it("renders status badge", () => {
    render(<ProjectCard project={makeProject({ status: "active" })} />);
    expect(screen.getByText("active")).toBeInTheDocument();
  });

  it("renders different status badges", () => {
    render(<ProjectCard project={makeProject({ status: "paused" })} />);
    expect(screen.getByText("paused")).toBeInTheDocument();
  });

  it("renders form type", () => {
    render(<ProjectCard project={makeProject({ form: "factorial" })} />);
    expect(screen.getByText("factorial")).toBeInTheDocument();
  });

  it("renders objective", () => {
    render(<ProjectCard project={makeProject({ objective: "record" })} />);
    expect(screen.getByText("record")).toBeInTheDocument();
  });

  it("renders tested count with commas", () => {
    render(<ProjectCard project={makeProject({ total_tested: 50000 })} />);
    expect(screen.getByText("50,000 tested")).toBeInTheDocument();
  });

  it("renders found count", () => {
    render(<ProjectCard project={makeProject({ total_found: 12 })} />);
    expect(screen.getByText("12 found")).toBeInTheDocument();
  });

  it("renders best digits when greater than zero", () => {
    render(<ProjectCard project={makeProject({ best_digits: 45678 })} />);
    expect(screen.getByText("best: 45,678 digits")).toBeInTheDocument();
  });

  it("hides best digits when zero", () => {
    render(<ProjectCard project={makeProject({ best_digits: 0 })} />);
    expect(screen.queryByText(/best:/)).not.toBeInTheDocument();
  });

  it("renders cost when greater than zero", () => {
    render(<ProjectCard project={makeProject({ total_cost_usd: 3.25 })} />);
    expect(screen.getByText("$3.25")).toBeInTheDocument();
  });

  it("hides cost when zero", () => {
    render(<ProjectCard project={makeProject({ total_cost_usd: 0 })} />);
    expect(screen.queryByText(/\$/)).not.toBeInTheDocument();
  });

  it("shows Activate button for draft projects", () => {
    render(<ProjectCard project={makeProject({ status: "draft" })} />);
    expect(screen.getByText("Activate")).toBeInTheDocument();
  });

  it("shows Resume button for paused projects", () => {
    render(<ProjectCard project={makeProject({ status: "paused" })} />);
    expect(screen.getByText("Resume")).toBeInTheDocument();
  });

  it("shows Pause button for active projects", () => {
    render(<ProjectCard project={makeProject({ status: "active" })} />);
    expect(screen.getByText("Pause")).toBeInTheDocument();
  });

  it("hides action buttons for completed projects", () => {
    render(<ProjectCard project={makeProject({ status: "completed" })} />);
    expect(screen.queryByText("Activate")).not.toBeInTheDocument();
    expect(screen.queryByText("Resume")).not.toBeInTheDocument();
    expect(screen.queryByText("Pause")).not.toBeInTheDocument();
  });

  it("renders link to project detail page", () => {
    render(<ProjectCard project={makeProject({ slug: "my-project" })} />);
    const links = screen.getAllByRole("link");
    const detailLink = links.find(
      (l) => l.getAttribute("href") === "/projects/?slug=my-project"
    );
    expect(detailLink).toBeDefined();
  });

  it("renders checkbox when onToggleSelect is provided", () => {
    const onToggle = vi.fn();
    render(
      <ProjectCard
        project={makeProject()}
        selected={false}
        onToggleSelect={onToggle}
      />
    );
    expect(screen.getByRole("checkbox")).toBeInTheDocument();
  });

  it("does not render checkbox when onToggleSelect is not provided", () => {
    render(<ProjectCard project={makeProject()} />);
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });
});
