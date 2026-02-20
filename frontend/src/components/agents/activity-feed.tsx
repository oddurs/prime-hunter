/**
 * @module agents/activity-feed
 *
 * Real-time agent activity feed showing recent events across all tasks.
 * Each event displays an icon, summary, optional task ID, agent name,
 * and timestamp.
 */

import { Card, CardContent } from "@/components/ui/card";
import { EmptyState } from "@/components/empty-state";
import { useAgentEvents, type AgentEvent } from "@/hooks/use-agents";
import { formatTime } from "@/lib/format";
import { eventIcon } from "./helpers";

export function ActivityFeed() {
  const { events, loading } = useAgentEvents();

  if (loading) {
    return <EmptyState message="Loading activity..." />;
  }

  if (events.length === 0) {
    return <EmptyState message="No agent activity yet." />;
  }

  return (
    <div className="space-y-1">
      {events.map((ev: AgentEvent) => (
        <Card key={ev.id} className="py-2">
          <CardContent className="p-0 px-4 flex items-start gap-2">
            <div className="mt-0.5">{eventIcon(ev.event_type)}</div>
            <div className="min-w-0 flex-1 text-xs">
              <div className="flex items-center gap-2">
                <span className="font-medium text-foreground">{ev.summary}</span>
                {ev.task_id && (
                  <span className="text-muted-foreground">task #{ev.task_id}</span>
                )}
              </div>
              <div className="flex items-center gap-2 text-muted-foreground mt-0.5">
                {ev.agent && <span className="font-mono">{ev.agent}</span>}
                <span>{formatTime(ev.created_at)}</span>
              </div>
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
