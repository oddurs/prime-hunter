"use client";

/**
 * @module use-agent-memory
 *
 * React hooks and CRUD functions for agent knowledge storage.
 * The memory system lets agents persist patterns, conventions,
 * gotchas, and preferences across tasks. Memories are categorized
 * and keyed for upsert-based updates.
 *
 * Data source: REST API `/api/agents/memory` endpoint.
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export interface AgentMemory {
  id: number;
  key: string;
  value: string;
  category: string;
  created_by_task: number | null;
  created_at: string;
  updated_at: string;
}

const MEMORY_CATEGORIES = [
  "pattern",
  "convention",
  "gotcha",
  "preference",
  "architecture",
  "general",
] as const;

export type MemoryCategory = (typeof MEMORY_CATEGORIES)[number];
export { MEMORY_CATEGORIES };

export function useAgentMemory() {
  const [memories, setMemories] = useState<AgentMemory[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchMemories = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/agents/memory`);
      if (res.ok) {
        const data = await res.json();
        setMemories(data as AgentMemory[]);
      }
    } catch {
      // Network error — keep previous state
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchMemories();
  }, [fetchMemories]);

  return { memories, loading, refetch: fetchMemories };
}

export async function upsertMemory(
  key: string,
  value: string,
  category: string = "general"
) {
  const resp = await fetch(`${API_BASE}/api/agents/memory`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ key, value, category }),
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error(
      (body as Record<string, string>).error || "Failed to upsert memory"
    );
  }
  return (await resp.json()) as AgentMemory;
}

export async function deleteMemory(key: string) {
  const resp = await fetch(
    `${API_BASE}/api/agents/memory/${encodeURIComponent(key)}`,
    { method: "DELETE" }
  );
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error(
      (body as Record<string, string>).error || "Failed to delete memory"
    );
  }
}
