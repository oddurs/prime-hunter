"use client";

/**
 * @module use-projects
 *
 * React hooks for managing prime-hunting projects via Supabase.
 *
 * A **project** is a high-level campaign to discover primes of a specific form,
 * broken into ordered **phases** (each mapping to a search job with its own
 * parameters and block size). Projects track aggregate progress (total tested,
 * total found, best digit count, cost) and emit **events** for milestones,
 * phase transitions, and errors.
 *
 * Hooks:
 * - `useProjects()` — list all projects with realtime updates
 * - `useProject(slug)` — single project detail with phases and events
 *
 * Action functions:
 * - `activateProject(slug)` — start or resume a project
 * - `pauseProject(slug)` — pause a running project
 * - `cancelProject(slug)` — cancel a project permanently
 *
 * @see {@link src/search_manager.rs} — Rust-side project lifecycle
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";
import { API_BASE } from "@/lib/format";

/** Summary of a prime-hunting project (maps to `projects` table row). */
export interface ProjectSummary {
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
  total_cost_usd: number;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

/** A phase within a project — an ordered search step with its own parameters. */
export interface ProjectPhase {
  id: number;
  name: string;
  description: string;
  phase_order: number;
  status: string;
  search_params: Record<string, unknown> | null;
  block_size: number;
  total_tested: number;
  total_found: number;
  search_job_id: number | null;
  started_at: string | null;
  completed_at: string | null;
}

/** An event emitted by a project (milestone, phase change, error, etc.). */
export interface ProjectEvent {
  id: number;
  project_id: number;
  event_type: string;
  summary: string;
  detail: Record<string, unknown> | null;
  created_at: string;
}

/**
 * Fetch all projects from the `projects` table with realtime updates.
 *
 * Returns projects ordered by creation date (newest first), limited to 200.
 * Subscribes to Supabase Realtime for INSERT/UPDATE/DELETE on the table.
 */
export function useProjects() {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchProjects = useCallback(async () => {
    const { data, error: queryError } = await supabase
      .from("projects")
      .select("*")
      .order("created_at", { ascending: false })
      .limit(200);

    if (queryError) {
      setError(queryError.message);
    } else if (data) {
      setProjects(data as ProjectSummary[]);
      setError(null);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  // Realtime subscription for project changes
  useEffect(() => {
    const channel = supabase
      .channel("projects_changes")
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "projects" },
        () => {
          fetchProjects();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchProjects]);

  return { projects, loading, error, refetch: fetchProjects };
}

/**
 * Fetch a single project by slug, along with its phases and events.
 *
 * Phases are ordered by `phase_order` (ascending). Events are ordered
 * by `created_at` (newest first), limited to 200.
 *
 * Subscribes to Supabase Realtime for changes on all three tables
 * (filtered to the matching project).
 */
export function useProject(slug: string) {
  const [project, setProject] = useState<ProjectSummary | null>(null);
  const [phases, setPhases] = useState<ProjectPhase[]>([]);
  const [events, setEvents] = useState<ProjectEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchProject = useCallback(async () => {
    if (!slug) return;

    // Fetch project by slug
    const { data: projectData, error: projectError } = await supabase
      .from("projects")
      .select("*")
      .eq("slug", slug)
      .single();

    if (projectError) {
      setError(projectError.message);
      setLoading(false);
      return;
    }

    const proj = projectData as ProjectSummary;
    setProject(proj);
    setError(null);

    // Fetch phases for this project
    const { data: phaseData } = await supabase
      .from("project_phases")
      .select("*")
      .eq("project_id", proj.id)
      .order("phase_order", { ascending: true });

    if (phaseData) {
      setPhases(phaseData as ProjectPhase[]);
    }

    // Fetch events for this project
    const { data: eventData } = await supabase
      .from("project_events")
      .select("*")
      .eq("project_id", proj.id)
      .order("created_at", { ascending: false })
      .limit(200);

    if (eventData) {
      setEvents(eventData as ProjectEvent[]);
    }

    setLoading(false);
  }, [slug]);

  useEffect(() => {
    fetchProject();
  }, [fetchProject]);

  // Realtime subscriptions for project, phases, and events
  useEffect(() => {
    if (!slug) return;

    const channel = supabase
      .channel(`project_detail_${slug}`)
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "projects" },
        () => {
          fetchProject();
        }
      )
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "project_phases" },
        () => {
          fetchProject();
        }
      )
      .on(
        "postgres_changes",
        { event: "INSERT", schema: "public", table: "project_events" },
        () => {
          fetchProject();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchProject, slug]);

  return { project, phases, events, loading, error, refetch: fetchProject };
}

/**
 * Activate (start or resume) a project via the Rust backend API.
 *
 * Sends `POST /api/projects/{slug}/activate` to transition the project
 * from `draft` or `paused` to `active`.
 */
export async function activateProject(slug: string): Promise<void> {
  const resp = await fetch(`${API_BASE}/api/projects/${encodeURIComponent(slug)}/activate`, {
    method: "POST",
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error((body as Record<string, string>).error || "Failed to activate project");
  }
}

/**
 * Pause a running project via the Rust backend API.
 *
 * Sends `POST /api/projects/{slug}/pause` to transition the project
 * from `active` to `paused`, stopping work distribution.
 */
export async function pauseProject(slug: string): Promise<void> {
  const resp = await fetch(`${API_BASE}/api/projects/${encodeURIComponent(slug)}/pause`, {
    method: "POST",
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error((body as Record<string, string>).error || "Failed to pause project");
  }
}

/**
 * Cancel a project permanently via the Rust backend API.
 *
 * Sends `POST /api/projects/{slug}/cancel` to transition the project
 * to `cancelled`. This is irreversible — cancelled projects cannot be resumed.
 */
export async function cancelProject(slug: string): Promise<void> {
  const resp = await fetch(`${API_BASE}/api/projects/${encodeURIComponent(slug)}/cancel`, {
    method: "POST",
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error((body as Record<string, string>).error || "Failed to cancel project");
  }
}
