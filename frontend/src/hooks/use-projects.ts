"use client";

/**
 * @module use-projects
 *
 * React hooks for managing prime-hunting projects via the REST API.
 *
 * A **project** is a high-level campaign to discover primes of a specific form,
 * broken into ordered **phases** (each mapping to a search job with its own
 * parameters and block size). Projects track aggregate progress (total tested,
 * total found, best digit count, cost) and emit **events** for milestones,
 * phase transitions, and errors.
 *
 * Hooks:
 * - `useProjects()` -- list all projects with polling (every 10 seconds)
 * - `useProject(slug)` -- single project detail with phases and events
 *
 * Action functions:
 * - `activateProject(slug)` -- start or resume a project
 * - `pauseProject(slug)` -- pause a running project
 * - `cancelProject(slug)` -- cancel a project permanently
 *
 * @see {@link src/search_manager.rs} -- Rust-side project lifecycle
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

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

/** A phase within a project -- an ordered search step with its own parameters. */
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
 * Fetch all projects from the REST API with polling (every 10 seconds).
 *
 * Returns projects ordered by creation date (newest first).
 */
export function useProjects() {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchProjects = useCallback(async () => {
    try {
      const resp = await fetch(`${API_BASE}/api/projects`);
      if (resp.ok) {
        const body = await resp.json();
        setProjects(body.projects ?? []);
        setError(null);
      } else {
        setError(`Failed to fetch projects: ${resp.status}`);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to fetch projects");
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  // Poll every 10 seconds
  useEffect(() => {
    const interval = setInterval(fetchProjects, 10_000);
    return () => clearInterval(interval);
  }, [fetchProjects]);

  return { projects, loading, error, refetch: fetchProjects };
}

/**
 * Fetch a single project by slug, along with its phases and events.
 *
 * Phases are ordered by `phase_order` (ascending). Events are ordered
 * by `created_at` (newest first).
 *
 * Polls every 10 seconds.
 */
export function useProject(slug: string) {
  const [project, setProject] = useState<ProjectSummary | null>(null);
  const [phases, setPhases] = useState<ProjectPhase[]>([]);
  const [events, setEvents] = useState<ProjectEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchProject = useCallback(async () => {
    if (!slug) return;

    try {
      // Fetch project detail (includes phases)
      const resp = await fetch(`${API_BASE}/api/projects/${encodeURIComponent(slug)}`);
      if (!resp.ok) {
        setError(`Failed to fetch project: ${resp.status}`);
        setLoading(false);
        return;
      }

      const body = await resp.json();
      setProject(body as ProjectSummary);
      setPhases(body.phases ?? []);
      setError(null);

      // Fetch events separately
      try {
        const eventsResp = await fetch(`${API_BASE}/api/projects/${encodeURIComponent(slug)}/events`);
        if (eventsResp.ok) {
          const eventsBody = await eventsResp.json();
          setEvents(eventsBody.events ?? []);
        }
      } catch {
        /* ignore event fetch errors */
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to fetch project");
    }

    setLoading(false);
  }, [slug]);

  useEffect(() => {
    fetchProject();
  }, [fetchProject]);

  // Poll every 10 seconds
  useEffect(() => {
    if (!slug) return;
    const interval = setInterval(fetchProject, 10_000);
    return () => clearInterval(interval);
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
 * to `cancelled`. This is irreversible -- cancelled projects cannot be resumed.
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
