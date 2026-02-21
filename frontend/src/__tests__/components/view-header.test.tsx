/**
 * @file Tests for the ViewHeader component
 * @module __tests__/components/view-header
 *
 * Validates the page-level header component used on every page to display
 * the page title (as h1), optional subtitle, metadata slot, action buttons
 * slot, and tab navigation slot. Tests cover heading element semantics,
 * conditional subtitle/metadata/actions/tabs rendering, custom className,
 * and border divider presence.
 *
 * @see {@link ../../components/view-header} Source component
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ViewHeader } from "@/components/view-header";

// Tests the ViewHeader: title rendering (h1), subtitle, metadata slot,
// actions slot, tabs slot, className support, and border divider.
describe("ViewHeader", () => {
  it("renders title", () => {
    render(<ViewHeader title="Dashboard" />);
    expect(screen.getByText("Dashboard")).toBeInTheDocument();
  });

  it("renders title as h1 element", () => {
    render(<ViewHeader title="Fleet" />);
    const heading = screen.getByRole("heading", { level: 1 });
    expect(heading).toHaveTextContent("Fleet");
  });

  it("renders subtitle when provided", () => {
    render(
      <ViewHeader
        title="Browse"
        subtitle="Explore all discovered primes"
      />
    );
    expect(
      screen.getByText("Explore all discovered primes")
    ).toBeInTheDocument();
  });

  it("does not render subtitle when not provided", () => {
    render(<ViewHeader title="Browse" />);
    // Subtitle element should not exist
    expect(
      screen.queryByText("Explore all discovered primes")
    ).not.toBeInTheDocument();
  });

  it("renders metadata slot when provided", () => {
    render(
      <ViewHeader
        title="Searches"
        metadata={<span data-testid="meta">3 active</span>}
      />
    );
    expect(screen.getByTestId("meta")).toBeInTheDocument();
    expect(screen.getByText("3 active")).toBeInTheDocument();
  });

  it("renders actions slot when provided", () => {
    render(
      <ViewHeader
        title="Projects"
        actions={
          <button data-testid="action-btn">New Project</button>
        }
      />
    );
    expect(screen.getByTestId("action-btn")).toBeInTheDocument();
    expect(screen.getByText("New Project")).toBeInTheDocument();
  });

  it("renders tabs slot when provided", () => {
    render(
      <ViewHeader
        title="Agents"
        tabs={<div data-testid="tabs">Tab1 | Tab2</div>}
      />
    );
    expect(screen.getByTestId("tabs")).toBeInTheDocument();
  });

  it("applies custom className", () => {
    const { container } = render(
      <ViewHeader title="Test" className="custom-class" />
    );
    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).toContain("custom-class");
  });

  it("renders border-b divider when no tabs", () => {
    const { container } = render(<ViewHeader title="Test" />);
    const dividers = container.querySelectorAll(".border-b");
    expect(dividers.length).toBeGreaterThan(0);
  });
});
