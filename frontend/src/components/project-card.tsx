/**
 * @module project-card
 *
 * Summary card for a project in the project list. Shows status, form,
 * objective, progress metrics, cost, and action buttons.
 */

import Link from "next/link";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { numberWithCommas } from "@/lib/format";
import { API_BASE } from "@/lib/format";
import { toast } from "sonner";
import {
  ArrowRight,
  Play,
  Pause,
  XCircle,
  Target,
  Search,
  CheckCircle2,
  Beaker,
} from "lucide-react";

interface ProjectSummary {
  slug: string;
  name: string;
  form: string;
  objective: string;
  status: string;
  total_tested: number;
  total_found: number;
  best_digits: number;
  total_cost_usd: number;
}

const statusColors: Record<string, string> = {
  draft: "bg-muted text-muted-foreground",
  active: "bg-green-500/15 text-green-600 dark:text-green-400",
  paused: "bg-yellow-500/15 text-yellow-600 dark:text-yellow-400",
  completed: "bg-blue-500/15 text-blue-600 dark:text-blue-400",
  cancelled: "bg-muted text-muted-foreground",
  failed: "bg-red-500/15 text-red-600 dark:text-red-400",
};

const objectiveIcons: Record<string, typeof Target> = {
  record: Target,
  survey: Search,
  verification: CheckCircle2,
  custom: Beaker,
};

export function ProjectCard({
  project,
  selected,
  onToggleSelect,
}: {
  project: ProjectSummary;
  selected?: boolean;
  onToggleSelect?: (slug: string) => void;
}) {
  const ObjIcon = objectiveIcons[project.objective] ?? Beaker;

  async function activate() {
    try {
      const res = await fetch(
        `${API_BASE}/api/projects/${project.slug}/activate`,
        { method: "POST" }
      );
      if (res.ok) toast.success(`Project '${project.name}' activated`);
      else toast.error("Failed to activate project");
    } catch {
      toast.error("Failed to activate project");
    }
  }

  async function pause() {
    try {
      const res = await fetch(
        `${API_BASE}/api/projects/${project.slug}/pause`,
        { method: "POST" }
      );
      if (res.ok) toast.success(`Project '${project.name}' paused`);
      else toast.error("Failed to pause project");
    } catch {
      toast.error("Failed to pause project");
    }
  }

  return (
    <Card>
      <CardContent className="flex items-center gap-4 py-4">
        {onToggleSelect && (
          <input
            type="checkbox"
            checked={selected ?? false}
            onChange={() => onToggleSelect(project.slug)}
            className="h-4 w-4 rounded border-border accent-primary shrink-0"
          />
        )}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <Link
              href={`/projects/?slug=${project.slug}`}
              className="font-medium text-foreground hover:underline truncate"
            >
              {project.name}
            </Link>
            <Badge
              variant="outline"
              className={`text-xs ${statusColors[project.status] ?? ""}`}
            >
              {project.status}
            </Badge>
          </div>
          <div className="flex items-center gap-3 text-xs text-muted-foreground">
            <span className="flex items-center gap-1">
              <ObjIcon className="h-3 w-3" />
              {project.objective}
            </span>
            <span>{project.form}</span>
            <span>{numberWithCommas(project.total_tested)} tested</span>
            <span>{project.total_found} found</span>
            {project.best_digits > 0 && (
              <span>best: {numberWithCommas(project.best_digits)} digits</span>
            )}
            {project.total_cost_usd > 0 && (
              <span>${project.total_cost_usd.toFixed(2)}</span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-1">
          {project.status === "draft" && (
            <Button size="sm" variant="outline" onClick={activate}>
              <Play className="h-3 w-3 mr-1" />
              Activate
            </Button>
          )}
          {project.status === "paused" && (
            <Button size="sm" variant="outline" onClick={activate}>
              <Play className="h-3 w-3 mr-1" />
              Resume
            </Button>
          )}
          {project.status === "active" && (
            <Button size="sm" variant="outline" onClick={pause}>
              <Pause className="h-3 w-3 mr-1" />
              Pause
            </Button>
          )}
          <Link href={`/projects/?slug=${project.slug}`}>
            <Button size="sm" variant="ghost">
              <ArrowRight className="h-4 w-4" />
            </Button>
          </Link>
        </div>
      </CardContent>
    </Card>
  );
}
