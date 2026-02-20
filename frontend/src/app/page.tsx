"use client";

/**
 * @module page (Dashboard)
 *
 * Main dashboard page — the landing view after login. Displays:
 *
 * - **Stat cards**: total primes, largest prime, per-form counts
 * - **Recent primes table**: latest discoveries with form/digits/proof
 * - **Discovery timeline**: stacked area chart of primes over time
 * - **Digit distribution**: bar chart of primes by digit count
 * - **Throughput gauge**: real-time candidates/sec sparkline
 * - **Fleet summary**: active workers, total cores, search status
 *
 * Data sources: Supabase (stats, primes, charts) + WebSocket (fleet).
 */

import { useState } from "react";
import { useWs } from "@/contexts/websocket-context";
import { useStats } from "@/hooks/use-stats";
import { usePrimes } from "@/hooks/use-primes";
import { useTimeline } from "@/hooks/use-timeline";
import { useDistribution } from "@/hooks/use-distribution";
import { toast } from "sonner";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import Link from "next/link";
import { DiscoveryTimeline } from "@/components/charts/discovery-timeline";
import { DigitDistribution } from "@/components/charts/digit-distribution";
import { NewSearchDialog } from "@/components/new-search-dialog";
import { InsightCards } from "@/components/insight-cards";
import { FormLeaderboard } from "@/components/form-leaderboard";
import { ActivityFeed } from "@/components/activity-feed";
import { useFormLeaderboard } from "@/hooks/use-form-leaderboard";
import { AddServerDialog } from "@/components/add-server-dialog";
import { HostNodeCard, type HostNode } from "@/components/host-node-card";
import { ServiceStatusCard } from "@/components/service-status-card";
import { AgentControllerCard } from "@/components/agent-controller-card";
import { numberWithCommas } from "@/lib/format";
import type { WorkerStatus, ManagedSearch, Deployment, HardwareMetrics } from "@/hooks/use-websocket";
import { MetricsBar } from "@/components/metrics-bar";
import { WorkerDetailDialog } from "@/components/worker-detail-dialog";
import { PrimesTable } from "@/components/primes-table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ViewHeader } from "@/components/view-header";

function groupByHost(
  fleet: { workers: WorkerStatus[] } | null,
  coordinator: HardwareMetrics | null,
  searches: ManagedSearch[],
  deployments: Deployment[],
): HostNode[] {
  // Group workers by hostname
  const hostMap = new Map<string, WorkerStatus[]>();
  for (const w of fleet?.workers ?? []) {
    const key = w.hostname || w.worker_id;
    const list = hostMap.get(key) ?? [];
    list.push(w);
    hostMap.set(key, list);
  }

  // Determine unique hostnames
  const hostnames = [...hostMap.keys()];

  // Build host nodes
  const nodes: HostNode[] = [];
  let coordinatorMerged = false;

  for (const hostname of hostnames) {
    const workers = hostMap.get(hostname) ?? [];
    // Take hardware metrics from first worker that has them (identical for co-located)
    const workerMetrics = workers.find((w) => w.metrics)?.metrics ?? null;

    // If coordinator exists and there's only one unique hostname, merge coordinator metrics
    const isCoord = coordinator !== null && hostnames.length <= 1;
    const metrics = workerMetrics ?? (isCoord ? coordinator : null);
    if (isCoord) coordinatorMerged = true;

    // Match searches to this host's workers
    const workerIds = new Set(workers.map((w) => w.worker_id));
    const hostSearches = searches.filter((s) => workerIds.has(s.worker_id));
    const hostDeployments = deployments.filter((d) => d.hostname === hostname);

    nodes.push({
      hostname,
      isCoordinator: isCoord,
      metrics,
      workers,
      searches: hostSearches,
      deployments: hostDeployments,
      totalCores: workers.reduce((sum, w) => Math.max(sum, w.cores), 0),
      totalTested: workers.reduce((sum, w) => sum + w.tested, 0),
      totalFound: workers.reduce((sum, w) => sum + w.found, 0),
    });
  }

  // Standalone coordinator card if not merged and coordinator exists
  if (coordinator && !coordinatorMerged) {
    const orphanDeployments = deployments.filter(
      (d) => !hostMap.has(d.hostname),
    );
    nodes.unshift({
      hostname: "coordinator",
      isCoordinator: true,
      metrics: coordinator,
      workers: [],
      searches: [],
      deployments: orphanDeployments,
      totalCores: 0,
      totalTested: 0,
      totalFound: 0,
    });
  }

  // Sort: coordinator first, then by core count descending
  nodes.sort((a, b) => {
    if (a.isCoordinator !== b.isCoordinator) return a.isCoordinator ? -1 : 1;
    return b.totalCores - a.totalCores;
  });

  return nodes;
}

export default function Dashboard() {
  // Coordination data from WebSocket
  const {
    status,
    fleet,
    coordinator,
    searches,
    deployments,
    agentTasks,
    agentBudgets,
    runningAgents,
    connected,
  } = useWs();

  // Prime data from Supabase
  const { stats } = useStats();
  const { primes } = usePrimes();
  const { timeline } = useTimeline();
  const { distribution } = useDistribution();
  const { entries: leaderboardEntries } = useFormLeaderboard();

  const [newSearchOpen, setNewSearchOpen] = useState(false);
  const [addServerOpen, setAddServerOpen] = useState(false);
  const [workerDetailOpen, setWorkerDetailOpen] = useState(false);
  const [selectedWorker, setSelectedWorker] = useState<WorkerStatus | null>(null);

  const runningSearchCount = searches.filter((s) => s.status === "running").length;
  const hasCoordinatorActive = !!status?.active && !!status?.checkpoint;
  const dashboardRunningCount = runningSearchCount + (hasCoordinatorActive ? 1 : 0);
  const totalCores = fleet?.total_cores ?? 0;

  // Infrastructure grouping
  const hostNodes = groupByHost(fleet, coordinator, searches, deployments);
  const serverCount = hostNodes.filter((n) => n.workers.length > 0 || n.isCoordinator).length;

  function scrollToSection(id: string) {
    const section = document.getElementById(id);
    if (!section) return;
    section.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  return (
    <>
      <Tabs
        defaultValue="status-section"
        onValueChange={(v) => scrollToSection(v)}
      >
        <ViewHeader
          title="Dashboard"
          subtitle="Real-time prime search monitoring, fleet health, and discovery history"
          metadata={
            <div className="flex flex-wrap gap-1.5">
              <Badge variant="outline">
                {status?.active ? "Active search" : "Idle"}
              </Badge>
              <Badge variant="outline">
                {serverCount} server{serverCount !== 1 ? "s" : ""} &middot; {dashboardRunningCount} running &middot; {totalCores} cores
              </Badge>
            </div>
          }
          actions={
            <>
              <Button variant="outline" size="sm" asChild>
                <Link href="/browse">Browse</Link>
              </Button>
              <Button variant="outline" size="sm" asChild>
                <Link href="/searches">All Searches</Link>
              </Button>
              <Button size="sm" onClick={() => setNewSearchOpen(true)}>
                New Search
              </Button>
            </>
          }
          tabs={
            <TabsList variant="line">
              <TabsTrigger value="status-section">Status</TabsTrigger>
              <TabsTrigger value="infra-section">Infrastructure</TabsTrigger>
              <TabsTrigger value="insights-section">Insights</TabsTrigger>
              <TabsTrigger value="primes-section">Prime Archive</TabsTrigger>
            </TabsList>
          }
          className="mb-6"
        />
      </Tabs>

      {/* Status bar with progress */}
      <Card id="status-section" className="mb-6 py-3 scroll-mt-6">
        <CardContent className="p-0 px-4 space-y-2">
          <div className="text-xs font-medium text-muted-foreground">
            Current status
          </div>
          <div className="flex items-center gap-3">
            <div
              className={`size-2.5 rounded-full flex-shrink-0 ${
                status?.active
                  ? "bg-green-500"
                  : "bg-muted-foreground"
              }`}
            />
            <span className="text-sm flex-1">
              {status?.active ? (
                status?.checkpoint ? (
                  <>
                    <strong className="text-foreground">Active:</strong>{" "}
                    {status.checkpoint.type === "Factorial" &&
                      `Factorial search at n=${numberWithCommas(status.checkpoint.last_n ?? 0)}`}
                    {status.checkpoint.type === "Palindromic" &&
                      `Palindromic search at ${numberWithCommas(status.checkpoint.digit_count ?? 0)} digits`}
                    {status.checkpoint.type === "Kbn" &&
                      `k*b^n search at n=${numberWithCommas(status.checkpoint.last_n ?? 0)}`}
                    {!["Factorial", "Palindromic", "Kbn"].includes(
                      status.checkpoint.type
                    ) && "Search in progress"}
                  </>
                ) : (
                  <>
                    <strong className="text-foreground">Active:</strong>{" "}
                    {fleet && fleet.total_workers > 0
                      ? `${fleet.total_workers} worker${fleet.total_workers !== 1 ? "s" : ""} across ${fleet.total_cores} cores`
                      : "Fleet search in progress"}
                  </>
                )
              ) : (
                <>
                  <strong className="text-foreground">Idle</strong> — no active
                  search
                </>
              )}
            </span>
            {(() => {
              const cp = status?.checkpoint;
              if (!cp || !status?.active) return null;
              let pct: number | null = null;
              if (cp.type === "Factorial" && cp.start != null && cp.end != null && cp.last_n != null) {
                pct = ((cp.last_n - cp.start) / (cp.end - cp.start)) * 100;
              } else if (cp.type === "Kbn" && cp.min_n != null && cp.max_n != null && cp.last_n != null) {
                pct = ((cp.last_n - cp.min_n) / (cp.max_n - cp.min_n)) * 100;
              } else if (cp.type === "Palindromic" && cp.min_digits != null && cp.max_digits != null && cp.digit_count != null) {
                pct = ((cp.digit_count - cp.min_digits) / (cp.max_digits - cp.min_digits)) * 100;
              }
              if (pct == null) return null;
              pct = Math.min(100, Math.max(0, pct));
              return (
                <span className="text-xs text-muted-foreground tabular-nums">
                  {pct.toFixed(1)}%
                </span>
              );
            })()}
          </div>
          {(() => {
            const cp = status?.checkpoint;
            if (!cp || !status?.active) return null;
            let pct: number | null = null;
            if (cp.type === "Factorial" && cp.start != null && cp.end != null && cp.last_n != null) {
              pct = ((cp.last_n - cp.start) / (cp.end - cp.start)) * 100;
            } else if (cp.type === "Kbn" && cp.min_n != null && cp.max_n != null && cp.last_n != null) {
              pct = ((cp.last_n - cp.min_n) / (cp.max_n - cp.min_n)) * 100;
            } else if (cp.type === "Palindromic" && cp.min_digits != null && cp.max_digits != null && cp.digit_count != null) {
              pct = ((cp.digit_count - cp.min_digits) / (cp.max_digits - cp.min_digits)) * 100;
            }
            if (pct == null) return null;
            pct = Math.min(100, Math.max(0, pct));
            return (
              <div className="h-1.5 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-green-500 rounded-full transition-all duration-1000"
                  style={{ width: `${pct}%` }}
                />
              </div>
            );
          })()}
        </CardContent>
      </Card>

      {/* Infrastructure section */}
      <section id="infra-section" className="mb-6 scroll-mt-6">
        <div className="flex items-baseline justify-between mb-3 border-b pb-2">
          <h2 className="text-base font-semibold text-foreground">
            Infrastructure
          </h2>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => setAddServerOpen(true)}>
              Add Server
            </Button>
            <Button variant="outline" size="sm" onClick={() => setNewSearchOpen(true)}>
              New Search
            </Button>
          </div>
        </div>

        {/* Services row */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 mb-4">
          <ServiceStatusCard
            name="Coordinator"
            status={connected ? "online" : "offline"}
          >
            {coordinator && (
              <div className="space-y-1.5">
                <MetricsBar label="CPU" percent={coordinator.cpu_usage_percent} />
                <MetricsBar
                  label="Mem"
                  percent={coordinator.memory_usage_percent}
                  detail={`${coordinator.memory_used_gb} / ${coordinator.memory_total_gb} GB`}
                />
              </div>
            )}
            <div className="text-xs text-muted-foreground">
              {dashboardRunningCount} search{dashboardRunningCount !== 1 ? "es" : ""} running
            </div>
          </ServiceStatusCard>

          <AgentControllerCard
            runningAgents={runningAgents}
            agentTasks={agentTasks}
            agentBudgets={agentBudgets}
          />

          <ServiceStatusCard
            name="Database"
            status={stats ? "online" : "offline"}
          >
            <div className="text-xs text-muted-foreground">
              {stats ? `${numberWithCommas(stats.total)} primes stored` : "Connecting..."}
            </div>
            {stats?.by_form && stats.by_form.length > 0 && (
              <div className="text-xs text-muted-foreground">
                {stats.by_form.length} form{stats.by_form.length !== 1 ? "s" : ""} indexed
              </div>
            )}
          </ServiceStatusCard>
        </div>

        {/* Fleet aggregate stats strip */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-4">
          {[
            { value: serverCount, label: "Servers" },
            { value: totalCores, label: "Cores" },
            { value: dashboardRunningCount, label: "Active Searches" },
            { value: fleet?.total_tested ?? 0, label: "Candidates Tested" },
          ].map(({ value, label }) => (
            <div key={label} className="text-center py-2 rounded-md bg-muted/50">
              <div className="text-lg font-semibold tabular-nums text-foreground">
                {numberWithCommas(value)}
              </div>
              <div className="text-[11px] text-muted-foreground">{label}</div>
            </div>
          ))}
        </div>

        {/* Server nodes */}
        {(() => {
          const filteredNodes = hostNodes.filter(
            (n) => n.workers.length > 0 || n.deployments.length > 0 || (n.isCoordinator && n.metrics)
          );
          return filteredNodes.length === 0 ? (
            <Card className="py-8">
              <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
                No servers connected. Click &ldquo;Add Server&rdquo; to deploy a worker via SSH.
              </CardContent>
            </Card>
          ) : (
            <div className="space-y-3">
              {filteredNodes.map((node) => (
                <HostNodeCard
                  key={node.hostname}
                  node={node}
                  onInspectWorker={(w) => {
                    setSelectedWorker(w);
                    setWorkerDetailOpen(true);
                  }}
                />
              ))}
            </div>
          );
        })()}
      </section>

      <AddServerDialog
        open={addServerOpen}
        onOpenChange={setAddServerOpen}
        onDeployed={() => {
          toast.success("Worker deployment started");
        }}
      />

      <NewSearchDialog
        open={newSearchOpen}
        onOpenChange={setNewSearchOpen}
        onCreated={() => {
          toast.success("Search started");
        }}
      />

      <section id="insights-section" className="mb-6 scroll-mt-6">
        <div className="flex items-baseline justify-between mb-3 border-b pb-2">
          <div>
            <h2 className="text-base font-semibold text-foreground">Insights</h2>
            <p className="text-sm text-muted-foreground mt-1">
              Computed metrics, form analytics, and recent activity
            </p>
          </div>
        </div>

        {/* Insight cards row */}
        <InsightCards
          timeline={timeline}
          stats={stats}
          fleet={fleet}
          latestPrime={primes.primes[0] ?? null}
        />

        {/* Form leaderboard */}
        <FormLeaderboard entries={leaderboardEntries} />

        {/* Activity feed + Discovery timeline side by side */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-4">
          <ActivityFeed primes={primes.primes} />
          <DiscoveryTimeline data={timeline} />
        </div>

        {/* Digit distribution full width */}
        <DigitDistribution data={distribution} />
      </section>

      <PrimesTable stats={stats} />

      <WorkerDetailDialog
        worker={selectedWorker}
        open={workerDetailOpen}
        onOpenChange={(open) => {
          if (!open) {
            setWorkerDetailOpen(false);
            setSelectedWorker(null);
          }
        }}
      />
    </>
  );
}
