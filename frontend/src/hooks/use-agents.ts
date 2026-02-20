"use client";

/**
 * @module use-agents
 *
 * Barrel re-export for all agent-related hooks and functions.
 * The actual implementations live in focused modules:
 *
 * - {@link use-agent-tasks} — Task CRUD, events, templates, roles, observability
 * - {@link use-agent-budgets} — Budget tracking and cost analytics
 * - {@link use-agent-schedules} — Automated schedule management
 * - {@link use-agent-memory} — Agent knowledge store
 */

export {
  // Types
  type AgentTask,
  type AgentEvent,
  type AgentLog,
  type AgentTemplate,
  type TemplateStep,
  type TaskTreeNode,
  type AgentRole,
  // Hooks
  useAgentTasks,
  useAgentEvents,
  useAgentTemplates,
  useAgentRoles,
  useAgentLogs,
  useAgentTimeline,
  // Functions
  createTask,
  expandTemplate,
  buildTaskTree,
  cancelTask,
} from "./use-agent-tasks";

export {
  // Types
  type AgentBudget,
  type DailyCostRow,
  type TemplateCostRow,
  // Hooks
  useAgentBudgets,
  useAgentDailyCosts,
  useAgentTemplateCosts,
  useAgentAnomalies,
} from "./use-agent-budgets";

export {
  // Types
  type AgentSchedule,
  // Hooks
  useAgentSchedules,
  // Functions
  createSchedule,
  updateSchedule,
  deleteSchedule,
  toggleSchedule,
} from "./use-agent-schedules";

export {
  // Types
  type AgentMemory,
  type MemoryCategory,
  // Constants
  MEMORY_CATEGORIES,
  // Hooks
  useAgentMemory,
  // Functions
  upsertMemory,
  deleteMemory,
} from "./use-agent-memory";
