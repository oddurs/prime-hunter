import { API_BASE } from "@/lib/format";

export type ReleaseRow = {
  version: string;
  artifacts: unknown;
  notes: string | null;
  published_at: string;
  created_at: string;
};

export type ChannelRow = {
  channel: string;
  version: string;
  rollout_percent: number;
  updated_at: string;
};

export type EventRow = {
  id: number;
  channel: string;
  from_version: string | null;
  to_version: string;
  rollout_percent: number;
  changed_by: string | null;
  changed_at: string;
};

export type AdoptionRow = {
  worker_version: string | null;
  workers: number;
};

export type ReleasesListResponse = {
  releases: ReleaseRow[];
  channels: ChannelRow[];
};

export type EventsResponse = { events: EventRow[] };
export type HealthResponse = {
  active_hours: number;
  adoption: AdoptionRow[];
  channels: ChannelRow[];
};

export type ArtifactInput = {
  os: string;
  arch: string;
  url: string;
  sha256: string;
  sig_url?: string;
};

export const DEFAULT_ARTIFACT_JSON = JSON.stringify(
  [
    {
      os: "linux",
      arch: "x86_64",
      url: "https://example.com/darkreach-worker-v1.2.3-linux-x86_64.tar.gz",
      sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    },
  ],
  null,
  2
);

export async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, init);
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    const message =
      typeof body === "object" && body !== null && "error" in body
        ? String((body as { error: unknown }).error)
        : `HTTP ${res.status}`;
    throw new Error(message);
  }
  return body as T;
}

export function rolloutBadgeClass(percent: number): string {
  if (percent >= 100) return "bg-emerald-500/10 text-emerald-300 border-emerald-500/30";
  if (percent <= 0) return "bg-zinc-500/10 text-zinc-300 border-zinc-500/30";
  return "bg-amber-500/10 text-amber-300 border-amber-500/30";
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isValidUrl(value: string): boolean {
  try {
    const u = new URL(value);
    return u.protocol === "http:" || u.protocol === "https:";
  } catch {
    return false;
  }
}

export function validateArtifacts(
  value: unknown
): { ok: true; artifacts: ArtifactInput[] } | { ok: false; error: string } {
  if (!Array.isArray(value)) {
    return { ok: false, error: "Artifacts JSON must be an array" };
  }

  for (let i = 0; i < value.length; i++) {
    const item = value[i];
    if (!isObject(item)) {
      return { ok: false, error: `Artifact ${i + 1}: expected an object` };
    }
    const os = item.os;
    const arch = item.arch;
    const url = item.url;
    const sha256 = item.sha256;
    const sigUrl = item.sig_url;

    if (typeof os !== "string" || os.trim() === "") {
      return { ok: false, error: `Artifact ${i + 1}: os is required` };
    }
    if (typeof arch !== "string" || arch.trim() === "") {
      return { ok: false, error: `Artifact ${i + 1}: arch is required` };
    }
    if (typeof url !== "string" || !isValidUrl(url)) {
      return { ok: false, error: `Artifact ${i + 1}: url must be a valid http(s) URL` };
    }
    if (typeof sha256 !== "string" || !/^[a-fA-F0-9]{64}$/.test(sha256)) {
      return { ok: false, error: `Artifact ${i + 1}: sha256 must be 64 hex chars` };
    }
    if (sigUrl != null && (typeof sigUrl !== "string" || !isValidUrl(sigUrl))) {
      return { ok: false, error: `Artifact ${i + 1}: sig_url must be a valid http(s) URL` };
    }
  }

  return { ok: true, artifacts: value as ArtifactInput[] };
}

export function releasesWorkerUrl(limit = 100): string {
  return `${API_BASE}/api/releases/worker?limit=${limit}`;
}

export function releasesEventsUrl(limit = 50): string {
  return `${API_BASE}/api/releases/events?limit=${limit}`;
}

export function releasesHealthUrl(activeHours = 24): string {
  return `${API_BASE}/api/releases/health?active_hours=${activeHours}`;
}
