/**
 * @file Tests for the StatCard component
 * @module __tests__/components/stat-card
 *
 * Validates the reusable stat card component used on the Dashboard, Searches,
 * Network, and Agents pages to display key metrics. Tests cover string and
 * numeric value rendering, optional icon slot, zero value display, JSX value
 * support, and custom className propagation.
 *
 * @see {@link ../../components/stat-card} Source component
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatCard } from "@/components/stat-card";

// Tests the StatCard: string/numeric values, optional icon, zero value,
// JSX value content, and custom className support.
describe("StatCard", () => {
  it("renders label and value", () => {
    render(<StatCard label="Total Primes" value="1,234" />);
    expect(screen.getByText("Total Primes")).toBeInTheDocument();
    expect(screen.getByText("1,234")).toBeInTheDocument();
  });

  it("renders numeric value", () => {
    render(<StatCard label="Cores" value={128} />);
    expect(screen.getByText("Cores")).toBeInTheDocument();
    expect(screen.getByText("128")).toBeInTheDocument();
  });

  it("renders icon when provided", () => {
    render(
      <StatCard
        label="Active"
        value="3"
        icon={<span data-testid="test-icon">icon</span>}
      />
    );
    expect(screen.getByTestId("test-icon")).toBeInTheDocument();
  });

  it("does not render icon when not provided", () => {
    const { container } = render(<StatCard label="Count" value="0" />);
    // Only the label and value should be present, no icon placeholder
    expect(container.querySelectorAll("[data-testid]")).toHaveLength(0);
  });

  it("renders zero value", () => {
    render(<StatCard label="Errors" value={0} />);
    expect(screen.getByText("0")).toBeInTheDocument();
  });

  it("renders JSX value", () => {
    render(
      <StatCard
        label="Status"
        value={<span data-testid="jsx-value">Online</span>}
      />
    );
    expect(screen.getByTestId("jsx-value")).toBeInTheDocument();
    expect(screen.getByText("Online")).toBeInTheDocument();
  });

  it("applies custom className", () => {
    const { container } = render(
      <StatCard label="Test" value="1" className="custom-class" />
    );
    // The Card root element should have the custom class
    const card = container.firstChild as HTMLElement;
    expect(card.className).toContain("custom-class");
  });
});
