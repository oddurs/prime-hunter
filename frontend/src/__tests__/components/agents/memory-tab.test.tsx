/**
 * @file Tests for the agent MemoryTab component
 * @module __tests__/components/agents/memory-tab
 *
 * Validates the Memory tab on the Agents page, which displays a key-value
 * store of agent knowledge grouped by category (engine, frontend, ops, etc.).
 * Tests cover loading state, empty state, category grouping, entry key/value
 * rendering, entry count, "Add Memory" button, inline add form, and task
 * provenance display ("task #N").
 *
 * @see {@link ../../../components/agents/memory-tab} Source component
 * @see {@link ../../../hooks/use-agents} useAgentMemory hook, MEMORY_CATEGORIES
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const mockRefetch = vi.fn();
const mockUseAgentMemory = vi.fn();

vi.mock("@/hooks/use-agents", () => ({
  useAgentMemory: () => mockUseAgentMemory(),
  upsertMemory: vi.fn().mockResolvedValue(undefined),
  deleteMemory: vi.fn().mockResolvedValue(undefined),
  MEMORY_CATEGORIES: ["general", "engine", "frontend", "ops"],
}));

vi.mock("@/components/empty-state", () => ({
  EmptyState: ({ message }: { message: string }) => (
    <div data-testid="empty-state">{message}</div>
  ),
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

import { MemoryTab } from "@/components/agents/memory-tab";

// Tests the MemoryTab: loading/empty states, category grouping, key/value display,
// entry count, add form, and task provenance.
describe("MemoryTab", () => {
  it("shows loading state", () => {
    mockUseAgentMemory.mockReturnValue({
      memories: [],
      loading: true,
      refetch: mockRefetch,
    });
    render(<MemoryTab />);
    expect(screen.getByText("Loading memory...")).toBeInTheDocument();
  });

  it("shows empty state when no memories", () => {
    mockUseAgentMemory.mockReturnValue({
      memories: [],
      loading: false,
      refetch: mockRefetch,
    });
    render(<MemoryTab />);
    expect(
      screen.getByText(
        "No agent memories yet. Agents will accumulate knowledge as they work."
      )
    ).toBeInTheDocument();
  });

  it("renders memory entries grouped by category", () => {
    mockUseAgentMemory.mockReturnValue({
      memories: [
        {
          id: 1,
          key: "proth_test_skip",
          value: "Skip base when a % p == 0",
          category: "engine",
          created_by_task: 42,
        },
        {
          id: 2,
          key: "dashboard_theme",
          value: "Use dark mode by default",
          category: "frontend",
          created_by_task: null,
        },
      ],
      loading: false,
      refetch: mockRefetch,
    });
    render(<MemoryTab />);
    expect(screen.getByText("engine")).toBeInTheDocument();
    expect(screen.getByText("frontend")).toBeInTheDocument();
    expect(screen.getByText("proth_test_skip")).toBeInTheDocument();
    expect(screen.getByText("Skip base when a % p == 0")).toBeInTheDocument();
    expect(screen.getByText("dashboard_theme")).toBeInTheDocument();
  });

  it("displays entry count", () => {
    mockUseAgentMemory.mockReturnValue({
      memories: [
        {
          id: 1,
          key: "test_key",
          value: "test_value",
          category: "general",
          created_by_task: null,
        },
      ],
      loading: false,
      refetch: mockRefetch,
    });
    render(<MemoryTab />);
    // The text includes an mdash entity
    expect(screen.getByText(/1 entries/)).toBeInTheDocument();
  });

  it("shows Add Memory button", () => {
    mockUseAgentMemory.mockReturnValue({
      memories: [],
      loading: false,
      refetch: mockRefetch,
    });
    render(<MemoryTab />);
    expect(screen.getByText("Add Memory")).toBeInTheDocument();
  });

  it("opens add form when Add Memory is clicked", async () => {
    const user = userEvent.setup();
    mockUseAgentMemory.mockReturnValue({
      memories: [],
      loading: false,
      refetch: mockRefetch,
    });
    render(<MemoryTab />);
    await user.click(screen.getByText("Add Memory"));
    expect(screen.getByPlaceholderText("e.g. proth_test_base_skip")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("What should agents know?")).toBeInTheDocument();
    expect(screen.getByText("Save")).toBeInTheDocument();
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("shows task ID when memory was created by a task", () => {
    mockUseAgentMemory.mockReturnValue({
      memories: [
        {
          id: 1,
          key: "test_key",
          value: "test_value",
          category: "general",
          created_by_task: 42,
        },
      ],
      loading: false,
      refetch: mockRefetch,
    });
    render(<MemoryTab />);
    expect(screen.getByText("task #42")).toBeInTheDocument();
  });
});
