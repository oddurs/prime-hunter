/**
 * @module agents/memory-tab
 *
 * Agent memory/knowledge management panel. Displays key-value memories
 * grouped by category, with add, edit, and delete support. Memories
 * are injected into agent system prompts at spawn time.
 */

import { useState } from "react";
import { toast } from "sonner";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { EmptyState } from "@/components/empty-state";
import {
  useAgentMemory,
  upsertMemory,
  deleteMemory,
  MEMORY_CATEGORIES,
  type MemoryCategory,
  type AgentMemory,
} from "@/hooks/use-agents";
import { Plus, Pencil, Trash2 } from "lucide-react";

export function MemoryTab() {
  const { memories, loading, refetch } = useAgentMemory();
  const [adding, setAdding] = useState(false);
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");
  const [newCategory, setNewCategory] = useState<MemoryCategory>("general");
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");

  async function handleAdd() {
    if (!newKey.trim() || !newValue.trim()) return;
    try {
      await upsertMemory(newKey.trim(), newValue.trim(), newCategory);
      toast.success("Memory added");
      setNewKey("");
      setNewValue("");
      setNewCategory("general");
      setAdding(false);
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to add memory");
    }
  }

  async function handleUpdate(key: string) {
    if (!editValue.trim()) return;
    try {
      const mem = memories.find((m) => m.key === key);
      await upsertMemory(key, editValue.trim(), mem?.category || "general");
      toast.success("Memory updated");
      setEditingKey(null);
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update");
    }
  }

  async function handleDelete(key: string) {
    try {
      await deleteMemory(key);
      toast.success("Memory deleted");
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete");
    }
  }

  if (loading) {
    return <EmptyState message="Loading memory..." />;
  }

  // Group by category
  const grouped = memories.reduce(
    (acc, mem) => {
      (acc[mem.category] ??= []).push(mem);
      return acc;
    },
    {} as Record<string, AgentMemory[]>
  );

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-xs text-muted-foreground">
          {memories.length} entries &mdash; injected into agent system prompts at spawn time
        </p>
        <Button size="sm" variant="outline" onClick={() => setAdding(!adding)}>
          <Plus className="size-3.5 mr-1" />
          Add Memory
        </Button>
      </div>

      {adding && (
        <Card className="py-3">
          <CardContent className="p-0 px-4 space-y-2">
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="text-xs font-medium text-muted-foreground">Key</label>
                <Input
                  value={newKey}
                  onChange={(e) => setNewKey(e.target.value)}
                  placeholder="e.g. proth_test_base_skip"
                  className="h-8"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">Category</label>
                <Select value={newCategory} onValueChange={(v) => setNewCategory(v as MemoryCategory)}>
                  <SelectTrigger className="h-8">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {MEMORY_CATEGORIES.map((c) => (
                      <SelectItem key={c} value={c}>
                        {c}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">Value</label>
              <textarea
                className="flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring min-h-[60px] resize-y"
                value={newValue}
                onChange={(e) => setNewValue(e.target.value)}
                placeholder="What should agents know?"
              />
            </div>
            <div className="flex gap-2">
              <Button size="sm" onClick={handleAdd} disabled={!newKey.trim() || !newValue.trim()}>
                Save
              </Button>
              <Button size="sm" variant="outline" onClick={() => setAdding(false)}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {memories.length === 0 && !adding ? (
        <EmptyState message="No agent memories yet. Agents will accumulate knowledge as they work." />
      ) : (
        Object.entries(grouped).map(([category, items]) => (
          <div key={category}>
            <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
              {category}
            </h3>
            <div className="space-y-1.5">
              {items.map((mem) => (
                <Card key={mem.id} className="py-2">
                  <CardContent className="p-0 px-4">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium font-mono text-foreground">
                            {mem.key}
                          </span>
                          {mem.created_by_task && (
                            <span className="text-[10px] text-muted-foreground">
                              task #{mem.created_by_task}
                            </span>
                          )}
                        </div>
                        {editingKey === mem.key ? (
                          <div className="flex items-center gap-2 mt-1">
                            <Input
                              value={editValue}
                              onChange={(e) => setEditValue(e.target.value)}
                              className="h-7 text-xs flex-1"
                            />
                            <Button size="xs" onClick={() => handleUpdate(mem.key)}>
                              Save
                            </Button>
                            <Button
                              size="xs"
                              variant="outline"
                              onClick={() => setEditingKey(null)}
                            >
                              Cancel
                            </Button>
                          </div>
                        ) : (
                          <p className="text-xs text-muted-foreground mt-0.5">{mem.value}</p>
                        )}
                      </div>
                      <div className="flex items-center gap-1 flex-shrink-0">
                        <button
                          onClick={() => {
                            setEditingKey(mem.key);
                            setEditValue(mem.value);
                          }}
                          className="text-muted-foreground hover:text-foreground transition-colors p-1"
                        >
                          <Pencil className="size-3" />
                        </button>
                        <button
                          onClick={() => handleDelete(mem.key)}
                          className="text-muted-foreground hover:text-red-500 transition-colors p-1"
                        >
                          <Trash2 className="size-3" />
                        </button>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          </div>
        ))
      )}
    </div>
  );
}
