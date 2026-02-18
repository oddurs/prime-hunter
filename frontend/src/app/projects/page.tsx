"use client";

/**
 * @module projects/page
 *
 * Project management page. Lists all prime-hunting projects (campaigns)
 * with status filters, progress tracking, and creation controls.
 * When accessed with `?slug=<slug>`, shows the project detail view
 * with phases, events, and cost tracking.
 */

import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "next/navigation";
import Link from "next/link";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ViewHeader } from "@/components/view-header";
import { NewProjectDialog } from "@/components/new-project-dialog";
import { ProjectCard } from "@/components/project-card";
import { RecordComparison } from "@/components/record-comparison";
import { PhaseTimeline } from "@/components/phase-timeline";
import { CostTracker } from "@/components/cost-tracker";
import { useWs } from "@/contexts/websocket-context";
import { API_BASE, numberWithCommas, formatTime } from "@/lib/format";
import {
  ArrowLeft,
  FolderOpen,
  Plus,
  Trophy,
  Activity,
  Play,
  Pause,
  XCircle,
  Loader2,
} from "lucide-react";

export default function ProjectsPage() {
  return (
    <Suspense>
      <ProjectsPageInner />
    </Suspense>
  );
}

function ProjectsPageInner() {
  const searchParams = useSearchParams();
  const slugParam = searchParams.get("slug");

  if (slugParam) {
    return <ProjectDetail slug={slugParam} />;
  }
  return <ProjectList />;
}

// ── Project List View ───────────────────────────────────────────

function ProjectList() {
  const { projects, records } = useWs();
  const [dialogOpen, setDialogOpen] = useState(false);

  const active = useMemo(
    () => projects.filter((p) => p.status === "active"),
    [projects]
  );
  const draft = useMemo(
    () => projects.filter((p) => p.status === "draft"),
    [projects]
  );
  const paused = useMemo(
    () => projects.filter((p) => p.status === "paused"),
    [projects]
  );
  const completed = useMemo(
    () =>
      projects.filter((p) =>
        ["completed", "cancelled", "failed"].includes(p.status)
      ),
    [projects]
  );
  const totalFound = useMemo(
    () => projects.reduce((sum, p) => sum + (p.total_found ?? 0), 0),
    [projects]
  );

  return (
    <div className="container mx-auto max-w-6xl px-4 py-6">
      <ViewHeader
        title="Projects"
        subtitle="Campaign-style prime discovery management"
        metadata={
          <div className="flex gap-4 text-sm text-muted-foreground">
            <span className="flex items-center gap-1">
              <Activity className="h-4 w-4" />
              {active.length} active
            </span>
            <span className="flex items-center gap-1">
              <FolderOpen className="h-4 w-4" />
              {projects.length} total
            </span>
            <span className="flex items-center gap-1">
              <Trophy className="h-4 w-4" />
              {numberWithCommas(totalFound)} primes found
            </span>
          </div>
        }
        actions={
          <Button size="sm" onClick={() => setDialogOpen(true)}>
            <Plus className="h-4 w-4 mr-1" />
            New Project
          </Button>
        }
      />

      {records.length > 0 && (
        <div className="mb-6">
          <h2 className="text-sm font-medium text-muted-foreground mb-3">
            World Records vs Our Best
          </h2>
          <div className="grid gap-3 grid-cols-1 md:grid-cols-2 lg:grid-cols-4">
            {records.map((r) => (
              <RecordComparison key={r.form} record={r} />
            ))}
          </div>
        </div>
      )}

      <Tabs defaultValue="active" className="mt-4">
        <TabsList>
          <TabsTrigger value="active">
            Active {active.length > 0 && `(${active.length})`}
          </TabsTrigger>
          <TabsTrigger value="draft">
            Draft {draft.length > 0 && `(${draft.length})`}
          </TabsTrigger>
          <TabsTrigger value="paused">
            Paused {paused.length > 0 && `(${paused.length})`}
          </TabsTrigger>
          <TabsTrigger value="completed">
            Completed {completed.length > 0 && `(${completed.length})`}
          </TabsTrigger>
          <TabsTrigger value="all">All ({projects.length})</TabsTrigger>
        </TabsList>

        {(["active", "draft", "paused", "completed", "all"] as const).map(
          (tab) => {
            const items =
              tab === "all"
                ? projects
                : tab === "completed"
                  ? completed
                  : tab === "active"
                    ? active
                    : tab === "draft"
                      ? draft
                      : paused;
            return (
              <TabsContent key={tab} value={tab} className="space-y-3 mt-4">
                {items.length === 0 ? (
                  <EmptyState message={`No ${tab} projects`} />
                ) : (
                  items.map((p) => <ProjectCard key={p.slug} project={p} />)
                )}
              </TabsContent>
            );
          }
        )}
      </Tabs>

      <NewProjectDialog open={dialogOpen} onOpenChange={setDialogOpen} />
    </div>
  );
}

// ── Project Detail View ─────────────────────────────────────────

interface ProjectDetailData {
  project: {
    id: number;
    slug: string;
    name: string;
    description: string;
    objective: string;
    form: string;
    status: string;
    total_tested: number;
    total_found: number;
    best_digits: number;
    total_core_hours: number;
    total_cost_usd: number;
    budget: Record<string, number>;
    created_at: string;
    started_at: string | null;
    completed_at: string | null;
  };
  phases: Array<{
    id: number;
    name: string;
    description: string;
    phase_order: number;
    status: string;
    total_tested: number;
    total_found: number;
    search_job_id: number | null;
    started_at: string | null;
    completed_at: string | null;
  }>;
  events: Array<{
    id: number;
    event_type: string;
    summary: string;
    created_at: string;
  }>;
}

function ProjectDetail({ slug }: { slug: string }) {
  const [data, setData] = useState<ProjectDetailData | null>(null);
  const [loading, setLoading] = useState(true);
  const [actioning, setActioning] = useState(false);

  const fetchProject = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/projects/${slug}`);
      if (res.ok) {
        const json = await res.json();
        setData(json);
      }
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [slug]);

  useEffect(() => {
    fetchProject();
    const interval = setInterval(fetchProject, 5000);
    return () => clearInterval(interval);
  }, [fetchProject]);

  async function doAction(action: string) {
    setActioning(true);
    try {
      const res = await fetch(`${API_BASE}/api/projects/${slug}/${action}`, {
        method: "POST",
      });
      if (res.ok) {
        toast.success(`Project ${action}d`);
        fetchProject();
      } else {
        const err = await res.json();
        toast.error(err.error || `Failed to ${action}`);
      }
    } catch {
      toast.error(`Failed to ${action}`);
    } finally {
      setActioning(false);
    }
  }

  if (loading) {
    return (
      <div className="container mx-auto max-w-6xl px-4 py-6">
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      </div>
    );
  }

  if (!data) {
    return (
      <div className="container mx-auto max-w-6xl px-4 py-6">
        <p className="text-muted-foreground">Project not found</p>
        <Link href="/projects" className="text-sm underline mt-2">
          Back to projects
        </Link>
      </div>
    );
  }

  const { project, phases, events } = data;
  const maxCostUsd = project.budget?.max_cost_usd ?? null;

  return (
    <div className="container mx-auto max-w-6xl px-4 py-6">
      <ViewHeader
        title={project.name}
        subtitle={project.description || `${project.objective} / ${project.form}`}
        metadata={
          <div className="flex gap-3 text-sm">
            <Badge variant="outline">{project.status}</Badge>
            <Badge variant="outline">{project.objective}</Badge>
            <Badge variant="outline">{project.form}</Badge>
          </div>
        }
        actions={
          <div className="flex items-center gap-2">
            {(project.status === "draft" || project.status === "paused") && (
              <Button
                size="sm"
                onClick={() => doAction("activate")}
                disabled={actioning}
              >
                <Play className="h-3 w-3 mr-1" />
                {project.status === "paused" ? "Resume" : "Activate"}
              </Button>
            )}
            {project.status === "active" && (
              <Button
                size="sm"
                variant="outline"
                onClick={() => doAction("pause")}
                disabled={actioning}
              >
                <Pause className="h-3 w-3 mr-1" />
                Pause
              </Button>
            )}
            {!["completed", "cancelled", "failed"].includes(project.status) && (
              <Button
                size="sm"
                variant="outline"
                onClick={() => doAction("cancel")}
                disabled={actioning}
              >
                <XCircle className="h-3 w-3 mr-1" />
                Cancel
              </Button>
            )}
            <Link href="/projects">
              <Button size="sm" variant="ghost">
                <ArrowLeft className="h-4 w-4 mr-1" />
                Back
              </Button>
            </Link>
          </div>
        }
      />

      <div className="grid gap-6 lg:grid-cols-3 mt-4">
        {/* Left column: phases + events */}
        <div className="lg:col-span-2 space-y-6">
          {/* Phases */}
          <Card>
            <CardContent className="py-4">
              <h3 className="text-sm font-medium mb-4">
                Phases ({phases.length})
              </h3>
              <PhaseTimeline phases={phases} />
            </CardContent>
          </Card>

          {/* Progress stats */}
          <div className="grid grid-cols-3 gap-4">
            <Card>
              <CardContent className="py-4 text-center">
                <p className="text-2xl font-bold font-mono">
                  {numberWithCommas(project.total_tested)}
                </p>
                <p className="text-xs text-muted-foreground">Tested</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="py-4 text-center">
                <p className="text-2xl font-bold font-mono text-green-500">
                  {project.total_found}
                </p>
                <p className="text-xs text-muted-foreground">Found</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="py-4 text-center">
                <p className="text-2xl font-bold font-mono">
                  {project.best_digits > 0
                    ? numberWithCommas(project.best_digits)
                    : "-"}
                </p>
                <p className="text-xs text-muted-foreground">Best Digits</p>
              </CardContent>
            </Card>
          </div>

          {/* Events */}
          {events.length > 0 && (
            <Card>
              <CardContent className="py-4">
                <h3 className="text-sm font-medium mb-3">Activity Log</h3>
                <div className="space-y-2 max-h-64 overflow-y-auto">
                  {events.map((evt) => (
                    <div
                      key={evt.id}
                      className="flex gap-3 text-xs border-b last:border-0 pb-2"
                    >
                      <span className="text-muted-foreground whitespace-nowrap">
                        {formatTime(evt.created_at)}
                      </span>
                      <Badge variant="outline" className="text-[10px]">
                        {evt.event_type}
                      </Badge>
                      <span className="text-foreground">{evt.summary}</span>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}
        </div>

        {/* Right column: cost + metadata */}
        <div className="space-y-4">
          <CostTracker
            totalCostUsd={project.total_cost_usd}
            maxCostUsd={maxCostUsd}
            totalCoreHours={project.total_core_hours}
            totalTested={project.total_tested}
          />

          <Card>
            <CardContent className="py-4">
              <h3 className="text-sm font-medium mb-3">Details</h3>
              <dl className="space-y-2 text-xs">
                <div className="flex justify-between">
                  <dt className="text-muted-foreground">Created</dt>
                  <dd>{formatTime(project.created_at)}</dd>
                </div>
                {project.started_at && (
                  <div className="flex justify-between">
                    <dt className="text-muted-foreground">Started</dt>
                    <dd>{formatTime(project.started_at)}</dd>
                  </div>
                )}
                {project.completed_at && (
                  <div className="flex justify-between">
                    <dt className="text-muted-foreground">Completed</dt>
                    <dd>{formatTime(project.completed_at)}</dd>
                  </div>
                )}
                <div className="flex justify-between">
                  <dt className="text-muted-foreground">Slug</dt>
                  <dd className="font-mono">{project.slug}</dd>
                </div>
              </dl>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <Card>
      <CardContent className="py-8 text-center text-muted-foreground">
        {message}
      </CardContent>
    </Card>
  );
}
