/**
 * @file Tests for the EmptyState placeholder component
 * @module __tests__/components/empty-state
 *
 * Validates the reusable EmptyState component shown across the dashboard
 * when a section has no data (e.g. no primes, no servers, no deployments).
 * Tests cover message text rendering, custom className propagation,
 * dashed border styling, and centered text alignment.
 *
 * @see {@link ../../components/empty-state} Source component
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { EmptyState } from "@/components/empty-state";

// Tests the EmptyState component: message rendering, custom class support,
// dashed border styling, and text centering.
describe("EmptyState", () => {
  /** Verifies the provided message string renders in the component. */
  it("renders the message text", () => {
    render(<EmptyState message="No primes found yet." />);
    expect(screen.getByText("No primes found yet.")).toBeInTheDocument();
  });

  /** Verifies different message content renders correctly (with special characters). */
  it("renders a different message", () => {
    render(<EmptyState message='No servers connected. Click "Add Server" to deploy a worker via SSH.' />);
    expect(
      screen.getByText('No servers connected. Click "Add Server" to deploy a worker via SSH.')
    ).toBeInTheDocument();
  });

  /** Verifies custom className is forwarded to the root card element. */
  it("applies custom className", () => {
    const { container } = render(
      <EmptyState message="Empty" className="my-custom-class" />
    );
    const card = container.firstChild as HTMLElement;
    expect(card.className).toContain("my-custom-class");
  });

  /** Verifies the card uses a dashed border to visually distinguish from data cards. */
  it("has dashed border styling", () => {
    const { container } = render(<EmptyState message="Nothing here" />);
    const card = container.firstChild as HTMLElement;
    expect(card.className).toContain("border-dashed");
  });

  /** Verifies the message text is centered within the card content area. */
  it("centers the message text", () => {
    const { container } = render(<EmptyState message="Centered" />);
    // The CardContent wrapper should have text-center class
    const content = container.querySelector(".text-center");
    expect(content).not.toBeNull();
    expect(content?.textContent).toBe("Centered");
  });
});
