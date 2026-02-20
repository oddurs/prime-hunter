/**
 * @module agents/budget-cards
 *
 * Budget period cards with progress bars, inline editing, and
 * spend/token tracking. Supports daily, weekly, and monthly periods.
 */

import { useState } from "react";
import { toast } from "sonner";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState } from "@/components/empty-state";
import { useAgentBudgets } from "@/hooks/use-agents";
import { Pencil } from "lucide-react";
import { numberWithCommas } from "@/lib/format";

export function BudgetCards() {
  const { budgets, loading, refetch } = useAgentBudgets();
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editValue, setEditValue] = useState("");

  if (loading) {
    return <EmptyState message="Loading budgets..." />;
  }

  if (budgets.length === 0) {
    return <EmptyState message="No budgets configured." />;
  }

  async function handleSave(id: number) {
    const val = parseFloat(editValue);
    if (isNaN(val) || val < 0) {
      toast.error("Invalid budget amount");
      return;
    }
    const { error } = await (await import("@/lib/supabase")).supabase
      .from("agent_budgets")
      .update({ budget_usd: val, updated_at: new Date().toISOString() })
      .eq("id", id);

    if (error) {
      toast.error(error.message);
    } else {
      toast.success("Budget updated");
      refetch();
    }
    setEditingId(null);
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
      {budgets.map((b) => {
        const pct = b.budget_usd > 0 ? Math.min((b.spent_usd / b.budget_usd) * 100, 100) : 0;
        const barColor =
          pct >= 90 ? "bg-red-500" : pct >= 70 ? "bg-amber-500" : "bg-green-500";

        return (
          <Card key={b.id} className="py-4">
            <CardContent className="p-0 px-4 space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm font-semibold capitalize">{b.period}</span>
                <button
                  onClick={() => {
                    setEditingId(b.id);
                    setEditValue(b.budget_usd.toString());
                  }}
                  className="text-muted-foreground hover:text-foreground transition-colors"
                >
                  <Pencil className="size-3.5" />
                </button>
              </div>

              {/* Progress bar */}
              <div>
                <div className="flex justify-between text-xs text-muted-foreground mb-1">
                  <span>${b.spent_usd.toFixed(2)} spent</span>
                  <span>${b.budget_usd.toFixed(2)} budget</span>
                </div>
                <div className="h-2 w-full rounded-full bg-muted overflow-hidden">
                  <div
                    className={`h-full rounded-full transition-all ${barColor}`}
                    style={{ width: `${pct}%` }}
                  />
                </div>
              </div>

              <div className="flex justify-between text-xs text-muted-foreground">
                <span>{numberWithCommas(b.tokens_used)} tokens</span>
                <span>since {new Date(b.period_start).toLocaleDateString()}</span>
              </div>

              {editingId === b.id && (
                <div className="flex items-center gap-2 pt-1 border-t">
                  <Input
                    type="number"
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    className="h-7 text-xs"
                    step="0.01"
                    min="0"
                  />
                  <Button size="xs" onClick={() => handleSave(b.id)}>
                    Save
                  </Button>
                  <Button size="xs" variant="outline" onClick={() => setEditingId(null)}>
                    Cancel
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
