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

import { useState, useEffect, useCallback } from "react";
import { useWs } from "@/contexts/websocket-context";
import { useStats } from "@/hooks/use-stats";
import { usePrimes, type PrimeFilter } from "@/hooks/use-primes";
import { useTimeline } from "@/hooks/use-timeline";
import { useDistribution } from "@/hooks/use-distribution";
import { toast } from "sonner";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
import { API_BASE, numberWithCommas, formToSlug, formatUptime } from "@/lib/format";
import type { WorkerStatus, ManagedSearch, Deployment, HardwareMetrics } from "@/hooks/use-websocket";
import { MetricsBar } from "@/components/metrics-bar";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ViewHeader } from "@/components/view-header";

type SortColumn = "expression" | "form" | "digits" | "found_at" | undefined;
type SortDir = "asc" | "desc" | undefined;

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
  const { primes, selectedPrime, fetchPrimes, fetchPrimeDetail, clearSelectedPrime } = usePrimes();
  const { timeline } = useTimeline();
  const { distribution } = useDistribution();
  const { entries: leaderboardEntries } = useFormLeaderboard();

  const [newSearchOpen, setNewSearchOpen] = useState(false);
  const [addServerOpen, setAddServerOpen] = useState(false);
  const [workerDetailOpen, setWorkerDetailOpen] = useState(false);
  const [selectedWorker, setSelectedWorker] = useState<WorkerStatus | null>(null);

  const [searchInput, setSearchInput] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [formFilter, setFormFilter] = useState<string>("");
  const [sortBy, setSortBy] = useState<SortColumn>(undefined);
  const [sortDir, setSortDir] = useState<SortDir>(undefined);
  const [detailOpen, setDetailOpen] = useState(false);

  const offset = primes.offset;
  const limit = primes.limit;
  const total = primes.total;
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

  // Debounce search input
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(searchInput);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchInput]);

  // Build current filter
  const buildFilter = useCallback((): PrimeFilter => {
    const f: PrimeFilter = {};
    if (formFilter) f.form = formFilter;
    if (debouncedSearch) f.search = debouncedSearch;
    if (sortBy) {
      f.sort_by = sortBy;
      f.sort_dir = sortDir;
    }
    return f;
  }, [formFilter, debouncedSearch, sortBy, sortDir]);

  // Re-fetch when filters change
  useEffect(() => {
    fetchPrimes(0, limit, buildFilter());
  }, [debouncedSearch, formFilter, sortBy, sortDir, limit, fetchPrimes, buildFilter]);

  const hasActiveFilters = !!(formFilter || debouncedSearch || sortBy);

  function clearFilters() {
    setSearchInput("");
    setDebouncedSearch("");
    setFormFilter("");
    setSortBy(undefined);
    setSortDir(undefined);
  }

  function handleSort(column: SortColumn) {
    if (sortBy === column) {
      if (sortDir === "asc") {
        setSortDir("desc");
      } else {
        setSortBy(undefined);
        setSortDir(undefined);
      }
    } else {
      setSortBy(column);
      setSortDir("asc");
    }
  }

  function sortIndicator(column: SortColumn) {
    if (sortBy !== column) return " \u2195";
    return sortDir === "asc" ? " \u2191" : " \u2193";
  }

  function prevPage() {
    fetchPrimes(Math.max(0, offset - limit), limit, buildFilter());
  }

  function nextPage() {
    if (offset + limit < total) {
      fetchPrimes(offset + limit, limit, buildFilter());
    }
  }

  function handleRowClick(id: number) {
    fetchPrimeDetail(id);
    setDetailOpen(true);
  }

  function handleDetailClose(open: boolean) {
    if (!open) {
      setDetailOpen(false);
      clearSelectedPrime();
    }
  }

  function exportData(format: "csv" | "json") {
    const params = new URLSearchParams();
    params.set("format", format);
    if (formFilter) params.set("form", formFilter);
    if (debouncedSearch) params.set("search", debouncedSearch);
    if (sortBy) {
      params.set("sort_by", sortBy);
      if (sortDir) params.set("sort_dir", sortDir);
    }
    window.open(`${API_BASE}/api/export?${params.toString()}`, "_blank");
  }

  let parsedSearchParams: Record<string, unknown> | null = null;
  if (selectedPrime?.search_params) {
    try {
      parsedSearchParams = JSON.parse(selectedPrime.search_params);
    } catch {
      // leave as null
    }
  }

  const formatTime = (iso: string) => new Date(iso).toLocaleString();

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
              {status?.active && status?.checkpoint ? (
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
            (n) => n.workers.length > 0 || n.deployments.length > 0 || !n.isCoordinator
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

      <Card id="primes-section" className="mb-4 scroll-mt-6 border">
        <CardContent className="p-4">
          <div className="flex flex-wrap items-center justify-between gap-3 mb-3">
            <h2 className="text-base font-semibold text-foreground">
              {hasActiveFilters ? "Filtered primes" : "Recent primes"}
            </h2>
            <span className="text-sm text-muted-foreground">
              {total === 0
                ? "0 results"
                : `${offset + 1}-${Math.min(offset + limit, total)} of ${numberWithCommas(total)}`}
            </span>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <div className="relative flex-1 min-w-[200px] max-w-[300px]">
              <svg
                className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.3-4.3" />
              </svg>
              <Input
                placeholder="Search expressions..."
                value={searchInput}
                onChange={(e) => setSearchInput(e.target.value)}
                className="pl-9 pr-8"
              />
              {searchInput && (
                <button
                  onClick={() => setSearchInput("")}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  aria-label="Clear search"
                >
                  <svg
                    className="h-4 w-4"
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M18 6 6 18" />
                    <path d="m6 6 12 12" />
                  </svg>
                </button>
              )}
            </div>

            <Select
              value={formFilter || "all"}
              onValueChange={(v) => setFormFilter(v === "all" ? "" : v)}
            >
              <SelectTrigger className="w-[160px]">
                <SelectValue placeholder="All forms" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All forms</SelectItem>
                {stats?.by_form.map((f) => (
                  <SelectItem key={f.form} value={f.form}>
                    {f.form} ({f.count})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="outline" size="sm">
                  <svg
                    className="h-4 w-4 mr-1.5"
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                    <polyline points="7 10 12 15 17 10" />
                    <line x1="12" x2="12" y1="15" y2="3" />
                  </svg>
                  Export
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent>
                <DropdownMenuItem onClick={() => exportData("csv")}>
                  Export CSV
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => exportData("json")}>
                  Export JSON
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>

            {hasActiveFilters && (
              <Button variant="ghost" size="sm" onClick={clearFilters}>
                Clear filters
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Primes table */}
      <Card className="py-0 overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead
                className="text-xs font-medium text-muted-foreground cursor-pointer select-none hover:text-foreground"
                onClick={() => handleSort("expression")}
              >
                Expression{sortIndicator("expression")}
              </TableHead>
              <TableHead
                className="text-xs font-medium text-muted-foreground cursor-pointer select-none hover:text-foreground"
                onClick={() => handleSort("form")}
              >
                Form{sortIndicator("form")}
              </TableHead>
              <TableHead
                className="text-xs font-medium text-muted-foreground cursor-pointer select-none hover:text-foreground"
                onClick={() => handleSort("digits")}
              >
                Digits{sortIndicator("digits")}
              </TableHead>
              <TableHead
                className="text-xs font-medium text-muted-foreground cursor-pointer select-none hover:text-foreground"
                onClick={() => handleSort("found_at")}
              >
                Found{sortIndicator("found_at")}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {primes.primes.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={4}
                  className="text-center text-muted-foreground py-8"
                >
                  No primes found yet
                </TableCell>
              </TableRow>
            ) : (
              primes.primes.map((p) => (
                <TableRow
                  key={p.id}
                  className="cursor-pointer"
                  onClick={() => handleRowClick(p.id)}
                >
                  <TableCell className="font-mono text-primary">
                    {p.expression}
                  </TableCell>
                  <TableCell>
                    <Link
                      href={`/docs?doc=${formToSlug(p.form)}`}
                      onClick={(e) => e.stopPropagation()}
                    >
                      <Badge
                        variant="outline"
                        className="cursor-pointer hover:bg-secondary/50"
                      >
                        {p.form}
                      </Badge>
                    </Link>
                  </TableCell>
                  <TableCell>{numberWithCommas(p.digits)}</TableCell>
                  <TableCell className="text-muted-foreground">
                    {formatTime(p.found_at)}
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </Card>

      {/* Pagination */}
      <div className="flex items-center justify-between gap-4 mt-4">
        <Button
          variant="outline"
          size="sm"
          onClick={prevPage}
          disabled={offset === 0}
        >
          Previous
        </Button>
        <span className="text-sm text-muted-foreground">
          {total === 0
            ? "0"
            : `${offset + 1}-${Math.min(offset + limit, total)}`}{" "}
          of {numberWithCommas(total)}
        </span>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={nextPage}
            disabled={offset + limit >= total}
          >
            Next
          </Button>
        </div>
      </div>

      {/* Worker detail dialog */}
      <Dialog open={workerDetailOpen} onOpenChange={(open) => {
        if (!open) {
          setWorkerDetailOpen(false);
          setSelectedWorker(null);
        }
      }}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <div
                className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${
                  selectedWorker && selectedWorker.last_heartbeat_secs_ago < 30
                    ? "bg-green-500"
                    : selectedWorker && selectedWorker.last_heartbeat_secs_ago < 60
                      ? "bg-yellow-500"
                      : "bg-red-500"
                }`}
              />
              {selectedWorker?.hostname ?? "Worker"}
            </DialogTitle>
          </DialogHeader>
          {selectedWorker && (() => {
            const throughput = selectedWorker.uptime_secs > 0
              ? (selectedWorker.tested / selectedWorker.uptime_secs).toFixed(1)
              : "0.0";
            let parsedParams: Record<string, unknown> | null = null;
            try {
              parsedParams = JSON.parse(selectedWorker.search_params);
            } catch {
              // leave as null
            }
            let parsedCheckpoint: Record<string, unknown> | null = null;
            if (selectedWorker.checkpoint) {
              try {
                parsedCheckpoint = JSON.parse(selectedWorker.checkpoint);
              } catch {
                // leave as null
              }
            }
            return (
              <div className="space-y-4">
                <div className="grid grid-cols-2 gap-4 text-sm">
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Worker ID
                    </div>
                    <span className="font-mono text-xs">{selectedWorker.worker_id}</span>
                  </div>
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Search Type
                    </div>
                    <Badge variant="outline">{selectedWorker.search_type}</Badge>
                  </div>
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Cores
                    </div>
                    <span className="font-semibold">{selectedWorker.cores}</span>
                  </div>
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Uptime
                    </div>
                    <span>{formatUptime(selectedWorker.uptime_secs)}</span>
                  </div>
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Tested
                    </div>
                    <span className="font-semibold">{numberWithCommas(selectedWorker.tested)}</span>
                  </div>
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Found
                    </div>
                    <span className="font-semibold">{selectedWorker.found}</span>
                  </div>
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Throughput
                    </div>
                    <span>{throughput} candidates/sec</span>
                  </div>
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Heartbeat
                    </div>
                    <span>
                      {selectedWorker.last_heartbeat_secs_ago < 5
                        ? "just now"
                        : `${selectedWorker.last_heartbeat_secs_ago}s ago`}
                    </span>
                  </div>
                </div>
                {selectedWorker.metrics && (
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-2">
                      Hardware
                    </div>
                    <div className="space-y-2">
                      <MetricsBar
                        label="CPU"
                        percent={selectedWorker.metrics.cpu_usage_percent}
                      />
                      <MetricsBar
                        label="Memory"
                        percent={selectedWorker.metrics.memory_usage_percent}
                        detail={`${selectedWorker.metrics.memory_used_gb} / ${selectedWorker.metrics.memory_total_gb} GB`}
                      />
                      <MetricsBar
                        label="Disk"
                        percent={selectedWorker.metrics.disk_usage_percent}
                        detail={`${selectedWorker.metrics.disk_used_gb} / ${selectedWorker.metrics.disk_total_gb} GB`}
                      />
                      <div className="text-xs text-muted-foreground">
                        Load: {selectedWorker.metrics.load_avg_1m} / {selectedWorker.metrics.load_avg_5m} / {selectedWorker.metrics.load_avg_15m}
                      </div>
                    </div>
                  </div>
                )}
                {selectedWorker.current && (
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Current candidate
                    </div>
                    <span className="font-mono text-xs break-all">{selectedWorker.current}</span>
                  </div>
                )}
                {parsedParams && (
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Search parameters
                    </div>
                    <pre className="bg-muted rounded-md p-3 text-xs overflow-auto max-h-32">
                      {JSON.stringify(parsedParams, null, 2)}
                    </pre>
                  </div>
                )}
                {parsedCheckpoint && (
                  <div>
                    <div className="text-xs font-medium text-muted-foreground mb-1">
                      Checkpoint
                    </div>
                    <pre className="bg-muted rounded-md p-3 text-xs overflow-auto max-h-32">
                      {JSON.stringify(parsedCheckpoint, null, 2)}
                    </pre>
                  </div>
                )}
              </div>
            );
          })()}
        </DialogContent>
      </Dialog>

      {/* Prime detail dialog */}
      <Dialog open={detailOpen} onOpenChange={handleDetailClose}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle className="font-mono text-primary break-all">
              {selectedPrime?.expression ?? "Loading..."}
            </DialogTitle>
          </DialogHeader>
          {selectedPrime && (
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Form
                  </div>
                  <Badge variant="outline">{selectedPrime.form}</Badge>
                </div>
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Digits
                  </div>
                  <span className="font-semibold">
                    {numberWithCommas(selectedPrime.digits)}
                  </span>
                </div>
                <div className="col-span-2">
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Found at
                  </div>
                  <span>{formatTime(selectedPrime.found_at)}</span>
                </div>
              </div>
              {parsedSearchParams && (
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Search parameters
                  </div>
                  <pre className="bg-muted rounded-md p-3 text-xs overflow-auto max-h-48">
                    {JSON.stringify(parsedSearchParams, null, 2)}
                  </pre>
                </div>
              )}
              {selectedPrime.search_params && !parsedSearchParams && (
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Search parameters
                  </div>
                  <pre className="bg-muted rounded-md p-3 text-xs overflow-auto max-h-48">
                    {selectedPrime.search_params}
                  </pre>
                </div>
              )}
            </div>
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}
