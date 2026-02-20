"use client";

/**
 * @module use-agent-memory
 *
 * React hooks and CRUD functions for agent knowledge storage.
 * The memory system lets agents persist patterns, conventions,
 * gotchas, and preferences across tasks. Memories are categorized
 * and keyed for upsert-based updates.
 *
 * Data source: `agent_memory` table with Supabase realtime.
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";

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
    const { data, error } = await supabase
      .from("agent_memory")
      .select("*")
      .order("category")
      .order("key");

    if (!error && data) {
      setMemories(data as AgentMemory[]);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchMemories();
  }, [fetchMemories]);

  // Realtime subscription
  useEffect(() => {
    const channel = supabase
      .channel("agent_memory_changes")
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "agent_memory" },
        () => {
          fetchMemories();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchMemories]);

  return { memories, loading, refetch: fetchMemories };
}

export async function upsertMemory(
  key: string,
  value: string,
  category: string = "general"
) {
  const { data, error } = await supabase
    .from("agent_memory")
    .upsert({ key, value, category, updated_at: new Date().toISOString() }, { onConflict: "key" })
    .select()
    .single();

  if (error) throw error;
  return data as AgentMemory;
}

export async function deleteMemory(key: string) {
  const { error } = await supabase.from("agent_memory").delete().eq("key", key);
  if (error) throw error;
}
