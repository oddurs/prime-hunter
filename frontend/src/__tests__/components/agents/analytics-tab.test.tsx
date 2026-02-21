/**
 * @file Tests for the agent AnalyticsTab component
 * @module __tests__/components/agents/analytics-tab
 *
 * Validates the Analytics tab on the Agents page, which displays daily cost
 * charts, template efficiency tables, and anomaly detection. Tests cover
 * loading states for all three sections, empty state messages, section
 * headings, template cost table rendering, and anomaly table display.
 *
 * @see {@link ../../../components/agents/analytics-tab} Source component
 * @see {@link ../../../hooks/use-agents} useAgentDailyCosts, useAgentTemplateCosts, useAgentAnomalies
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

const mockUseAgentDailyCosts = vi.fn();
const mockUseAgentTemplateCosts = vi.fn();
const mockUseAgentAnomalies = vi.fn();

vi.mock("@/hooks/use-agents", () => ({
  useAgentDailyCosts: (...args: unknown[]) => mockUseAgentDailyCosts(...args),
  useAgentTemplateCosts: () => mockUseAgentTemplateCosts(),
  useAgentAnomalies: (...args: unknown[]) => mockUseAgentAnomalies(...args),
}));

vi.mock("recharts", () => ({
  BarChart: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="bar-chart">{children}</div>
  ),
  Bar: () => null,
  XAxis: () => null,
  YAxis: () => null,
  Tooltip: () => null,
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  Legend: () => null,
}));

vi.mock("@/lib/format", () => ({
  numberWithCommas: (x: number) => String(x),
}));

import { AnalyticsTab } from "@/components/agents/analytics-tab";

// Tests the AnalyticsTab: loading states, empty states, section headings,
// template cost table with cost and token data, and anomaly detection table.
describe("AnalyticsTab", () => {
  it("shows loading states", () => {
    mockUseAgentDailyCosts.mockReturnValue({ data: [], loading: true });
    mockUseAgentTemplateCosts.mockReturnValue({ data: [], loading: true });
    mockUseAgentAnomalies.mockReturnValue({ data: [], loading: true });
    render(<AnalyticsTab />);
    const loadingElements = screen.getAllByText("Loading...");
    expect(loadingElements.length).toBe(3);
  });

  it("shows empty states when no data", () => {
    mockUseAgentDailyCosts.mockReturnValue({ data: [], loading: false });
    mockUseAgentTemplateCosts.mockReturnValue({ data: [], loading: false });
    mockUseAgentAnomalies.mockReturnValue({ data: [], loading: false });
    render(<AnalyticsTab />);
    expect(screen.getByText("No cost data yet")).toBeInTheDocument();
    expect(screen.getByText("No template data yet")).toBeInTheDocument();
    expect(screen.getByText("No anomalies detected")).toBeInTheDocument();
  });

  it("renders section headings", () => {
    mockUseAgentDailyCosts.mockReturnValue({ data: [], loading: false });
    mockUseAgentTemplateCosts.mockReturnValue({ data: [], loading: false });
    mockUseAgentAnomalies.mockReturnValue({ data: [], loading: false });
    render(<AnalyticsTab />);
    expect(screen.getByText("Daily Cost (Last 30 Days)")).toBeInTheDocument();
    expect(screen.getByText("Template Efficiency")).toBeInTheDocument();
  });

  it("renders template cost table when data exists", () => {
    mockUseAgentDailyCosts.mockReturnValue({ data: [], loading: false });
    mockUseAgentTemplateCosts.mockReturnValue({
      data: [
        {
          template_name: "engine-optimization",
          task_count: 5,
          total_cost: 2.5,
          avg_cost: 0.5,
          avg_tokens: 25000,
        },
      ],
      loading: false,
    });
    mockUseAgentAnomalies.mockReturnValue({ data: [], loading: false });
    render(<AnalyticsTab />);
    expect(screen.getByText("engine-optimization")).toBeInTheDocument();
    expect(screen.getByText("$2.5000")).toBeInTheDocument();
    expect(screen.getByText("$0.5000")).toBeInTheDocument();
  });

  it("renders anomaly table when anomalies detected", () => {
    mockUseAgentDailyCosts.mockReturnValue({ data: [], loading: false });
    mockUseAgentTemplateCosts.mockReturnValue({ data: [], loading: false });
    mockUseAgentAnomalies.mockReturnValue({
      data: [
        {
          id: 99,
          title: "Runaway sieve",
          template_name: "engine-optimization",
          tokens_used: 500000,
          cost_usd: 15.0,
        },
      ],
      loading: false,
    });
    render(<AnalyticsTab />);
    expect(screen.getByText("Runaway sieve")).toBeInTheDocument();
    expect(screen.getByText("500000")).toBeInTheDocument();
    expect(screen.getByText("$15.0000")).toBeInTheDocument();
  });
});
