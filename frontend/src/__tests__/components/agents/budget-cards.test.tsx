/**
 * @file Tests for the agent BudgetCards component
 * @module __tests__/components/agents/budget-cards
 *
 * Validates the budget cards shown on the Agents page Budget tab. Each card
 * represents a budget period (daily, weekly, monthly) with spend/budget amounts
 * and token counts. Tests cover loading state, empty state, budget period
 * rendering, dollar formatting, token count display, and the inline edit
 * form triggered by clicking the pencil icon.
 *
 * @see {@link ../../../components/agents/budget-cards} Source component
 * @see {@link ../../../hooks/use-agents} useAgentBudgets hook
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const mockRefetch = vi.fn();
const mockUseAgentBudgets = vi.fn();

vi.mock("@/hooks/use-agents", () => ({
  useAgentBudgets: () => mockUseAgentBudgets(),
}));

vi.mock("@/components/empty-state", () => ({
  EmptyState: ({ message }: { message: string }) => (
    <div data-testid="empty-state">{message}</div>
  ),
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

vi.mock("@/lib/format", () => ({
  numberWithCommas: (x: number) => String(x),
}));

vi.mock("@/lib/supabase", () => ({
  supabase: {
    from: () => ({
      update: () => ({
        eq: () => Promise.resolve({ error: null }),
      }),
    }),
  },
}));

import { BudgetCards } from "@/components/agents/budget-cards";

// Tests the BudgetCards: loading/empty states, period labels, spend/budget
// amounts, token counts, and inline edit form activation.
describe("BudgetCards", () => {
  it("shows loading state", () => {
    mockUseAgentBudgets.mockReturnValue({
      budgets: [],
      loading: true,
      refetch: mockRefetch,
    });
    render(<BudgetCards />);
    expect(screen.getByText("Loading budgets...")).toBeInTheDocument();
  });

  it("shows empty state when no budgets", () => {
    mockUseAgentBudgets.mockReturnValue({
      budgets: [],
      loading: false,
      refetch: mockRefetch,
    });
    render(<BudgetCards />);
    expect(screen.getByText("No budgets configured.")).toBeInTheDocument();
  });

  it("renders budget cards with period names", () => {
    mockUseAgentBudgets.mockReturnValue({
      budgets: [
        {
          id: 1,
          period: "daily",
          budget_usd: 10.0,
          spent_usd: 3.5,
          tokens_used: 50000,
          period_start: "2026-01-01T00:00:00Z",
        },
        {
          id: 2,
          period: "weekly",
          budget_usd: 50.0,
          spent_usd: 25.0,
          tokens_used: 200000,
          period_start: "2026-01-01T00:00:00Z",
        },
      ],
      loading: false,
      refetch: mockRefetch,
    });
    render(<BudgetCards />);
    expect(screen.getByText("daily")).toBeInTheDocument();
    expect(screen.getByText("weekly")).toBeInTheDocument();
  });

  it("displays spend and budget amounts", () => {
    mockUseAgentBudgets.mockReturnValue({
      budgets: [
        {
          id: 1,
          period: "daily",
          budget_usd: 10.0,
          spent_usd: 3.5,
          tokens_used: 50000,
          period_start: "2026-01-01T00:00:00Z",
        },
      ],
      loading: false,
      refetch: mockRefetch,
    });
    render(<BudgetCards />);
    expect(screen.getByText("$3.50 spent")).toBeInTheDocument();
    expect(screen.getByText("$10.00 budget")).toBeInTheDocument();
  });

  it("displays token count", () => {
    mockUseAgentBudgets.mockReturnValue({
      budgets: [
        {
          id: 1,
          period: "daily",
          budget_usd: 10.0,
          spent_usd: 0,
          tokens_used: 50000,
          period_start: "2026-01-01T00:00:00Z",
        },
      ],
      loading: false,
      refetch: mockRefetch,
    });
    render(<BudgetCards />);
    expect(screen.getByText("50000 tokens")).toBeInTheDocument();
  });

  it("shows edit form when pencil icon is clicked", async () => {
    const user = userEvent.setup();
    mockUseAgentBudgets.mockReturnValue({
      budgets: [
        {
          id: 1,
          period: "daily",
          budget_usd: 10.0,
          spent_usd: 3.5,
          tokens_used: 50000,
          period_start: "2026-01-01T00:00:00Z",
        },
      ],
      loading: false,
      refetch: mockRefetch,
    });
    render(<BudgetCards />);
    // Click the pencil button (it's a button element)
    const editButton = screen.getByRole("button");
    await user.click(editButton);
    expect(screen.getByText("Save")).toBeInTheDocument();
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });
});
