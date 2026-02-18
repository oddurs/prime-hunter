"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useWs } from "@/contexts/websocket-context";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { SearchCard } from "@/components/search-card";
import { NewSearchDialog } from "@/components/new-search-dialog";
import type { ManagedSearch } from "@/hooks/use-websocket";
import Link from "next/link";
import { API_BASE, numberWithCommas } from "@/lib/format";
import { Activity, CheckCircle2, Hash, Server, XCircle } from "lucide-react";

function sortSearches(searches: ManagedSearch[]): ManagedSearch[] {
  return [...searches].sort((a, b) => {
    const aRunning = a.status === "running" ? 0 : 1;
    const bRunning = b.status === "running" ? 0 : 1;
    if (aRunning !== bRunning) return aRunning - bRunning;
    return new Date(b.started_at).getTime() - new Date(a.started_at).getTime();
  });
}

function formatActiveCheckpoint(
  checkpoint: NonNullable<ReturnType<typeof useWs>["status"]>["checkpoint"]
): string {
  if (!checkpoint) return "Active search in progress";
  if (checkpoint.type === "factorial") {
    return `factorial: n ${checkpoint.start ?? "?"}..${checkpoint.end ?? "?"}`;
  }
  if (checkpoint.type === "kbn") {
    return `kbn: k=${checkpoint.last_n ?? "?"}, n ${checkpoint.min_n ?? "?"}..${checkpoint.max_n ?? "?"}`;
  }
  if (checkpoint.type === "palindromic") {
    return `palindromic: digits ${checkpoint.min_digits ?? "?"}..${checkpoint.max_digits ?? "?"}, current ${checkpoint.half_value ?? "?"}`;
  }
  return `${checkpoint.type} search is active`;
}

export default function SearchesPage() {
  const { searches, status, fleet } = useWs();
  const [newSearchOpen, setNewSearchOpen] = useState(false);
  const [stoppingCoordinator, setStoppingCoordinator] = useState(false);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [progressProbe, setProgressProbe] = useState<
    Record<string, { lastTested: number; lastMovedAtMs: number }>
  >({});

  const running = searches.filter((s) => s.status === "running");
  const sorted = sortSearches(searches);
  const sortedRunning = sortSearches(running);
  const hasCoordinatorActive = !!status?.active && !!status?.checkpoint;
  const runningCount = sortedRunning.length + (hasCoordinatorActive ? 1 : 0);
  const totalCount = sorted.length + (hasCoordinatorActive ? 1 : 0);
  const workers = useMemo(() => fleet?.workers ?? [], [fleet]);
  const completedCount = useMemo(
    () => searches.filter((s) => s.status === "completed").length,
    [searches]
  );
  const failedCount = useMemo(
    () =>
      searches.filter(
        (s) => typeof s.status === "object" && "failed" in s.status
      ).length,
    [searches]
  );
  const totalFound = useMemo(
    () => searches.reduce((sum, s) => sum + (s.found ?? 0), 0),
    [searches]
  );
  const totalTested = useMemo(
    () => searches.reduce((sum, s) => sum + (s.tested ?? 0), 0),
    [searches]
  );
  const staleWorkers = useMemo(
    () => workers.filter((w) => w.last_heartbeat_secs_ago >= 60).length,
    [workers]
  );
  const activeCoordinatorWorker = useMemo(
    () =>
      workers.find(
        (w) => w.last_heartbeat_secs_ago < 60 && w.current && w.current.length > 0
      ) ?? workers[0] ?? null,
    [workers]
  );
  const workersRef = useRef(workers);

  useEffect(() => {
    workersRef.current = workers;
  }, [workers]);

  useEffect(() => {
    if (sortedRunning.length === 0) return;
    const interval = setInterval(() => setNowMs(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [sortedRunning.length]);

  useEffect(() => {
    const interval = setInterval(() => {
      const snapshot = workersRef.current;
      if (!snapshot.length) return;
      const now = Date.now();
      setProgressProbe((prev) => {
        const next = { ...prev };
        for (const worker of snapshot) {
          const prevEntry = prev[worker.worker_id];
          if (!prevEntry) {
            next[worker.worker_id] = {
              lastTested: worker.tested,
              lastMovedAtMs: now,
            };
            continue;
          }
          if (worker.tested > prevEntry.lastTested) {
            next[worker.worker_id] = {
              lastTested: worker.tested,
              lastMovedAtMs: now,
            };
          } else {
            next[worker.worker_id] = {
              ...prevEntry,
              lastTested: worker.tested,
            };
          }
        }
        return next;
      });
    }, 2000);
    return () => clearInterval(interval);
  }, []);

  const runningDiagnostics = useMemo(() => {
    return sortedRunning.map((search) => {
      const worker = workers.find((w) => w.worker_id === search.worker_id);
      const probe = progressProbe[search.worker_id];
      const stalledSecs =
        probe && worker ? Math.floor((nowMs - probe.lastMovedAtMs) / 1000) : null;

      let diagnosis = "Healthy";
      let diagnosisVariant: "default" | "secondary" | "destructive" | "outline" =
        "default";
      if (!worker) {
        diagnosis = "No worker heartbeat yet";
        diagnosisVariant = "outline";
      } else if (worker.last_heartbeat_secs_ago >= 60) {
        diagnosis = "Worker heartbeat stale";
        diagnosisVariant = "destructive";
      } else if (stalledSecs !== null && stalledSecs >= 120) {
        diagnosis = `No tested increase for ${stalledSecs}s`;
        diagnosisVariant = "secondary";
      }

      return { search, worker, stalledSecs, diagnosis, diagnosisVariant };
    });
  }, [sortedRunning, workers, progressProbe, nowMs]);

  async function handleStopCoordinatorSearch() {
    if (!activeCoordinatorWorker) return;
    setStoppingCoordinator(true);
    try {
      const res = await fetch(
        `${API_BASE}/api/fleet/workers/${encodeURIComponent(activeCoordinatorWorker.worker_id)}/stop`,
        { method: "POST" }
      );
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      toast.success("Stop command sent — search will stop after current block");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to stop search";
      toast.error(message);
    } finally {
      setStoppingCoordinator(false);
    }
  }

  const coordinatorSearchCard = hasCoordinatorActive ? (
    <Card className="py-3 border-primary/30 bg-muted/40">
      <CardContent className="p-0 px-4 space-y-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="size-2 rounded-full bg-green-500" />
            <span className="text-sm font-medium text-foreground">
              Active coordinator search
            </span>
          </div>
          {activeCoordinatorWorker && (
            <Button
              variant="outline"
              size="xs"
              className="text-red-600 hover:text-red-700"
              disabled={stoppingCoordinator}
              onClick={handleStopCoordinatorSearch}
            >
              {stoppingCoordinator ? "Stopping..." : "Stop"}
            </Button>
          )}
        </div>
        <p className="text-xs text-muted-foreground">
          {formatActiveCheckpoint(status?.checkpoint ?? null)}
        </p>
        {activeCoordinatorWorker && (
          <p className="text-xs text-muted-foreground">
            worker{" "}
            <span className="font-mono">{activeCoordinatorWorker.worker_id}</span>
            {activeCoordinatorWorker.current
              ? ` • ${activeCoordinatorWorker.current}`
              : ""}
          </p>
        )}
      </CardContent>
    </Card>
  ) : null;

  return (
    <>
      <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between mb-4 pb-4 border-b">
        <div>
          <h1 className="text-2xl font-semibold text-foreground">Searches</h1>
          <p className="text-sm text-muted-foreground mt-1">
            {runningCount} running &middot; {totalCount} total
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" size="sm" asChild>
            <Link href="/fleet">Fleet</Link>
          </Button>
          <Button size="sm" onClick={() => setNewSearchOpen(true)}>New Search</Button>
        </div>
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-5 gap-3 mb-4">
        <Card className="py-3">
          <CardContent className="px-4 p-0 flex items-center justify-between">
            <div>
              <div className="text-xs font-medium text-muted-foreground">Running</div>
              <div className="text-xl font-semibold tabular-nums">{runningCount}</div>
            </div>
            <Activity className="size-4 text-primary" />
          </CardContent>
        </Card>
        <Card className="py-3">
          <CardContent className="px-4 p-0 flex items-center justify-between">
            <div>
              <div className="text-xs font-medium text-muted-foreground">Completed</div>
              <div className="text-xl font-semibold tabular-nums">{completedCount}</div>
            </div>
            <CheckCircle2 className="size-4 text-green-500" />
          </CardContent>
        </Card>
        <Card className="py-3">
          <CardContent className="px-4 p-0 flex items-center justify-between">
            <div>
              <div className="text-xs font-medium text-muted-foreground">Failed</div>
              <div className="text-xl font-semibold tabular-nums">{failedCount}</div>
            </div>
            <XCircle className="size-4 text-red-500" />
          </CardContent>
        </Card>
        <Card className="py-3">
          <CardContent className="px-4 p-0 flex items-center justify-between">
            <div>
              <div className="text-xs font-medium text-muted-foreground">Primes found</div>
              <div className="text-xl font-semibold tabular-nums">{numberWithCommas(totalFound)}</div>
            </div>
            <Hash className="size-4 text-amber-500" />
          </CardContent>
        </Card>
        <Card className="py-3">
          <CardContent className="px-4 p-0 flex items-center justify-between">
            <div>
              <div className="text-xs font-medium text-muted-foreground">Tested</div>
              <div className="text-xl font-semibold tabular-nums">{numberWithCommas(totalTested)}</div>
            </div>
            <Server className="size-4 text-muted-foreground" />
          </CardContent>
        </Card>
      </div>

      <Tabs defaultValue="all">
        <TabsList variant="line" className="w-full justify-start border-b rounded-none bg-transparent p-0">
          <TabsTrigger value="all">
            All{totalCount > 0 ? ` (${totalCount})` : ""}
          </TabsTrigger>
          <TabsTrigger value="running">
            Running{runningCount > 0 ? ` (${runningCount})` : ""}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="running" className="mt-4">
          {sortedRunning.length > 0 && (
            <Card className="py-3 mb-3 border bg-muted/40">
              <CardContent className="p-0 px-4 space-y-2">
                <div className="text-xs font-semibold text-foreground">
                  Search Diagnostics
                </div>
                <p className="text-xs text-muted-foreground">
                  Raw output for managed searches is written to backend stderr.
                  In dev: <code className="ml-1">tail -f .dev/backend.log</code>
                </p>
                <p className="text-xs text-muted-foreground">
                  You can also inspect worker heartbeat/counters in{" "}
                  <Link href="/fleet" className="text-primary hover:underline">
                    Fleet
                  </Link>
                  .
                </p>
              </CardContent>
            </Card>
          )}
          {runningDiagnostics.length > 0 && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 mb-3">
              {runningDiagnostics.map((entry) => (
                <Card key={entry.search.id} className="py-3 border">
                  <CardContent className="p-0 px-4 space-y-2">
                    <div className="flex items-center justify-between">
                      <div className="text-sm font-medium text-foreground">
                        Search #{entry.search.id}
                      </div>
                      <Badge variant={entry.diagnosisVariant}>{entry.diagnosis}</Badge>
                    </div>
                    <div className="text-xs text-muted-foreground">
                      worker:{" "}
                      <span className="font-mono">
                        {entry.search.worker_id}
                      </span>
                      {entry.search.pid ? ` • pid ${entry.search.pid}` : ""}
                    </div>
                    {entry.worker ? (
                      <>
                        <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground">
                          <span>
                            heartbeat{" "}
                            {entry.worker.last_heartbeat_secs_ago < 5
                              ? "now"
                              : `${entry.worker.last_heartbeat_secs_ago}s ago`}
                          </span>
                          <span>
                            tested {numberWithCommas(entry.worker.tested)}
                          </span>
                          <span>found {entry.worker.found}</span>
                          <span>
                            {entry.stalledSecs !== null
                              ? `last test progress ${entry.stalledSecs}s ago`
                              : "progress unknown"}
                          </span>
                        </div>
                        <div className="text-xs text-muted-foreground truncate">
                          current: {entry.worker.current || "idle"}
                        </div>
                      </>
                    ) : (
                      <div className="text-xs text-muted-foreground">
                        Waiting for worker registration/heartbeat from the child process.
                      </div>
                    )}
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
          {sortedRunning.length === 0 && !hasCoordinatorActive ? (
            <Card className="py-8 border-dashed">
              <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
                No running searches. Click &ldquo;New Search&rdquo; to start one.
              </CardContent>
            </Card>
          ) : (
            <div className="space-y-2">
              {coordinatorSearchCard}
              {sortedRunning.map((s) => (
                <SearchCard key={s.id} search={s} />
              ))}
            </div>
          )}
        </TabsContent>

        <TabsContent value="all" className="mt-4">
          {sorted.length === 0 && !hasCoordinatorActive ? (
            <Card className="py-8 border-dashed">
              <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
                No searches yet. Click &ldquo;New Search&rdquo; to start hunting primes.
              </CardContent>
            </Card>
          ) : (
            <div className="space-y-2">
              {coordinatorSearchCard}
              {sorted.map((s) => (
                <SearchCard key={s.id} search={s} />
              ))}
            </div>
          )}
        </TabsContent>
      </Tabs>

      <NewSearchDialog
        open={newSearchOpen}
        onOpenChange={setNewSearchOpen}
        onCreated={() => {
          toast.success("Search started");
        }}
      />
    </>
  );
}
