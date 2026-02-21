/**
 * @file Tests for the CostTracker component
 * @module __tests__/components/cost-tracker
 *
 * Validates the cost tracking display used on the Projects page. The
 * CostTracker shows compute spend (USD), core-hours consumed, and
 * candidate count. When a budget cap (maxCostUsd) is set, it also
 * renders a progress bar with percentage. Tests cover dollar formatting,
 * core-hour display, candidate count with thousand separators, budget
 * bar visibility, percentage calculation, and the 100% cap for
 * over-budget scenarios.
 *
 * @see {@link ../../components/cost-tracker} Source component
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock format module — provides numberWithCommas for candidate count display.
vi.mock("@/lib/format", () => ({
  numberWithCommas: (x: number) =>
    x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ","),
}));

import { CostTracker } from "@/components/cost-tracker";

// Tests the CostTracker component: dollar formatting, core-hours, candidate
// counts, budget bar visibility/percentage, and over-budget capping.
describe("CostTracker", () => {
  /** Verifies the section heading renders. */
  it("renders the title 'Cost Tracking'", () => {
    render(
      <CostTracker
        totalCostUsd={0}
        maxCostUsd={null}
        totalCoreHours={0}
        totalTested={0}
      />
    );
    expect(screen.getByText("Cost Tracking")).toBeInTheDocument();
  });

  /** Verifies USD cost renders with dollar sign and two decimal places. */
  it("renders cost in dollar format", () => {
    render(
      <CostTracker
        totalCostUsd={12.75}
        maxCostUsd={null}
        totalCoreHours={5.3}
        totalTested={1000}
      />
    );
    expect(screen.getByText("$12.75")).toBeInTheDocument();
  });

  /** Verifies zero cost renders as "$0.00" rather than empty or "$0". */
  it("renders zero cost", () => {
    render(
      <CostTracker
        totalCostUsd={0}
        maxCostUsd={null}
        totalCoreHours={0}
        totalTested={0}
      />
    );
    expect(screen.getByText("$0.00")).toBeInTheDocument();
  });

  /** Verifies core-hours value renders as a plain number. */
  it("renders core-hours", () => {
    render(
      <CostTracker
        totalCostUsd={5.0}
        maxCostUsd={null}
        totalCoreHours={123.4}
        totalTested={5000}
      />
    );
    expect(screen.getByText("123.4")).toBeInTheDocument();
  });

  /** Verifies candidate count uses thousand separators via numberWithCommas. */
  it("renders candidate count with commas", () => {
    render(
      <CostTracker
        totalCostUsd={5.0}
        maxCostUsd={null}
        totalCoreHours={10.0}
        totalTested={50000}
      />
    );
    expect(screen.getByText("50,000")).toBeInTheDocument();
  });

  /** Verifies the budget bar and budget amount display when maxCostUsd is set. */
  it("shows budget when maxCostUsd is set", () => {
    render(
      <CostTracker
        totalCostUsd={5.0}
        maxCostUsd={20.0}
        totalCoreHours={10.0}
        totalTested={1000}
      />
    );
    expect(screen.getByText("$20.00")).toBeInTheDocument();
    expect(screen.getByText("Budget usage")).toBeInTheDocument();
  });

  /** Verifies the budget section is hidden when no budget cap is configured. */
  it("hides budget when maxCostUsd is null", () => {
    render(
      <CostTracker
        totalCostUsd={5.0}
        maxCostUsd={null}
        totalCoreHours={10.0}
        totalTested={1000}
      />
    );
    expect(screen.queryByText("Budget usage")).not.toBeInTheDocument();
  });

  /** Verifies the budget section is hidden when maxCostUsd is zero (no cap). */
  it("hides budget bar when maxCostUsd is zero", () => {
    render(
      <CostTracker
        totalCostUsd={5.0}
        maxCostUsd={0}
        totalCoreHours={10.0}
        totalTested={1000}
      />
    );
    expect(screen.queryByText("Budget usage")).not.toBeInTheDocument();
  });

  /** Verifies percentage calculation: $15 of $20 budget = 75.0%. */
  it("shows correct budget percentage", () => {
    render(
      <CostTracker
        totalCostUsd={15.0}
        maxCostUsd={20.0}
        totalCoreHours={10.0}
        totalTested={1000}
      />
    );
    expect(screen.getByText("75.0%")).toBeInTheDocument();
  });

  /** Verifies percentage caps at 100% when spend exceeds budget. */
  it("caps percentage at 100% when over budget", () => {
    render(
      <CostTracker
        totalCostUsd={25.0}
        maxCostUsd={20.0}
        totalCoreHours={10.0}
        totalTested={1000}
      />
    );
    expect(screen.getByText("100.0%")).toBeInTheDocument();
  });

  /** Verifies all four metric labels render: Spent, Budget, Core-hours, Candidates. */
  it("renders spent and budget labels", () => {
    render(
      <CostTracker
        totalCostUsd={5.0}
        maxCostUsd={20.0}
        totalCoreHours={10.0}
        totalTested={1000}
      />
    );
    expect(screen.getByText("Spent")).toBeInTheDocument();
    expect(screen.getByText("Budget")).toBeInTheDocument();
    expect(screen.getByText("Core-hours")).toBeInTheDocument();
    expect(screen.getByText("Candidates")).toBeInTheDocument();
  });
});
