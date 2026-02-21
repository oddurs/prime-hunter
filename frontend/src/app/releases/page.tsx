"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { RefreshCw, RotateCcw, Send } from "lucide-react";

import { ViewHeader } from "@/components/view-header";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import { API_BASE, formatTime, relativeTime } from "@/lib/format";
import {
  DEFAULT_ARTIFACT_JSON,
  ReleaseRow,
  ChannelRow,
  EventRow,
  AdoptionRow,
  ReleasesListResponse,
  EventsResponse,
  HealthResponse,
  fetchJson,
  releasesEventsUrl,
  releasesHealthUrl,
  releasesWorkerUrl,
  rolloutBadgeClass,
  validateArtifacts,
} from "@/app/releases/lib";

export default function ReleasesPage() {
  const [loading, setLoading] = useState(true);
  const [releases, setReleases] = useState<ReleaseRow[]>([]);
  const [channels, setChannels] = useState<ChannelRow[]>([]);
  const [events, setEvents] = useState<EventRow[]>([]);
  const [adoption, setAdoption] = useState<AdoptionRow[]>([]);

  const [channel, setChannel] = useState("stable");
  const [version, setVersion] = useState("");
  const [rolloutPercent, setRolloutPercent] = useState("100");
  const [changedBy, setChangedBy] = useState("dashboard");
  const [submitting, setSubmitting] = useState(false);
  const [publishVersion, setPublishVersion] = useState("");
  const [publishNotes, setPublishNotes] = useState("");
  const [publishAt, setPublishAt] = useState("");
  const [artifactJson, setArtifactJson] = useState(DEFAULT_ARTIFACT_JSON);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [list, eventsData, health] = await Promise.all([
        fetchJson<ReleasesListResponse>(releasesWorkerUrl(100)),
        fetchJson<EventsResponse>(releasesEventsUrl(50)),
        fetchJson<HealthResponse>(releasesHealthUrl(24)),
      ]);

      setReleases(list.releases);
      setChannels(list.channels);
      setEvents(eventsData.events);
      setAdoption(health.adoption);

      if (!version && list.releases.length > 0) {
        setVersion(list.releases[0].version);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to load release data";
      toast.error(message);
    } finally {
      setLoading(false);
    }
  }, [version]);

  useEffect(() => {
    void load();
  }, [load]);

  const channelMap = useMemo(() => {
    const map = new Map<string, ChannelRow>();
    for (const ch of channels) {
      map.set(ch.channel, ch);
    }
    return map;
  }, [channels]);

  async function submitRollout() {
    const pct = Number(rolloutPercent);
    if (!version.trim()) {
      toast.error("Version is required");
      return;
    }
    if (!Number.isFinite(pct) || pct < 0 || pct > 100) {
      toast.error("Rollout percent must be between 0 and 100");
      return;
    }

    setSubmitting(true);
    try {
      await fetchJson(`${API_BASE}/api/releases/rollout`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          channel,
          version,
          rollout_percent: pct,
          changed_by: changedBy.trim() || null,
        }),
      });
      toast.success(`Rolled ${channel} to ${version} (${pct}%)`);
      await load();
    } catch (error) {
      const message = error instanceof Error ? error.message : "Rollout failed";
      toast.error(message);
    } finally {
      setSubmitting(false);
    }
  }

  async function submitRollback() {
    setSubmitting(true);
    try {
      await fetchJson(`${API_BASE}/api/releases/rollback`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          channel,
          changed_by: changedBy.trim() || null,
        }),
      });
      toast.success(`Rolled back ${channel}`);
      await load();
    } catch (error) {
      const message = error instanceof Error ? error.message : "Rollback failed";
      toast.error(message);
    } finally {
      setSubmitting(false);
    }
  }

  async function submitReleaseUpsert() {
    if (!publishVersion.trim()) {
      toast.error("Release version is required");
      return;
    }

    let artifacts: unknown;
    try {
      artifacts = JSON.parse(artifactJson);
    } catch {
      toast.error("Artifacts must be valid JSON");
      return;
    }

    const validated = validateArtifacts(artifacts);
    if (!validated.ok) {
      toast.error(validated.error);
      return;
    }

    setSubmitting(true);
    try {
      await fetchJson(`${API_BASE}/api/releases/worker`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          version: publishVersion.trim(),
          artifacts: validated.artifacts,
          notes: publishNotes.trim() || null,
          published_at: publishAt.trim() || null,
        }),
      });
      toast.success(`Upserted release ${publishVersion.trim()}`);
      if (!version) {
        setVersion(publishVersion.trim());
      }
      await load();
    } catch (error) {
      const message = error instanceof Error ? error.message : "Release upsert failed";
      toast.error(message);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="space-y-6">
      <ViewHeader
        title="Releases"
        subtitle="Canary/ramp/rollback controls with worker adoption visibility"
        actions={
          <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
            <RefreshCw className="size-4" />
            Refresh
          </Button>
        }
      />

      <Card>
        <CardHeader>
          <CardTitle>Publish / Update Release</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-3 md:grid-cols-3">
            <Input
              value={publishVersion}
              onChange={(e) => setPublishVersion(e.target.value)}
              placeholder="Version (e.g. 1.2.3)"
            />
            <Input
              value={publishAt}
              onChange={(e) => setPublishAt(e.target.value)}
              placeholder="Published at (ISO8601, optional)"
            />
            <Input
              value={publishNotes}
              onChange={(e) => setPublishNotes(e.target.value)}
              placeholder="Notes (optional)"
            />
          </div>
          <Textarea
            value={artifactJson}
            onChange={(e) => setArtifactJson(e.target.value)}
            placeholder='[{"os":"linux","arch":"x86_64","url":"...","sha256":"..."}]'
            className="min-h-[180px] font-mono text-xs"
          />
          <div className="flex justify-end">
            <Button onClick={submitReleaseUpsert} disabled={submitting}>
              <Send className="size-4" />
              Upsert Release Metadata
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Rollout Controls</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-3 md:grid-cols-4">
            <Input
              value={channel}
              onChange={(e) => setChannel(e.target.value)}
              placeholder="Channel (stable/beta)"
            />
            <Input
              value={version}
              onChange={(e) => setVersion(e.target.value)}
              placeholder="Version"
              list="release-versions"
            />
            <Input
              value={rolloutPercent}
              onChange={(e) => setRolloutPercent(e.target.value)}
              placeholder="Rollout %"
              type="number"
              min={0}
              max={100}
            />
            <Input
              value={changedBy}
              onChange={(e) => setChangedBy(e.target.value)}
              placeholder="Changed by"
            />
            <datalist id="release-versions">
              {releases.map((r) => (
                <option key={r.version} value={r.version} />
              ))}
            </datalist>
          </div>

          <div className="flex flex-wrap gap-2">
            <Button onClick={submitRollout} disabled={submitting}>
              <Send className="size-4" />
              Apply Rollout
            </Button>
            <Button variant="destructive" onClick={submitRollback} disabled={submitting}>
              <RotateCcw className="size-4" />
              Rollback Channel
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Channel Targets</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {channels.length === 0 ? (
              <p className="text-sm text-muted-foreground">No channels configured yet.</p>
            ) : (
              channels.map((ch) => (
                <div
                  key={ch.channel}
                  className="flex items-center justify-between rounded-md border p-3"
                >
                  <div>
                    <p className="text-sm font-medium">{ch.channel}</p>
                    <p className="text-xs text-muted-foreground">
                      {ch.version} • updated {relativeTime(ch.updated_at)}
                    </p>
                  </div>
                  <Badge className={rolloutBadgeClass(ch.rollout_percent)}>
                    {ch.rollout_percent}%
                  </Badge>
                </div>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Active Adoption (24h)</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {adoption.length === 0 ? (
              <p className="text-sm text-muted-foreground">No active worker heartbeats yet.</p>
            ) : (
              adoption.map((row) => {
                const ver = row.worker_version ?? "unknown";
                const targeted = Array.from(channelMap.values()).some((c) => c.version === ver);
                return (
                  <div
                    key={`${ver}-${row.workers}`}
                    className="flex items-center justify-between rounded-md border p-3"
                  >
                    <div>
                      <p className="text-sm font-medium">{ver}</p>
                      <p className="text-xs text-muted-foreground">{row.workers} active workers</p>
                    </div>
                    {targeted ? <Badge variant="secondary">Targeted</Badge> : <Badge variant="outline">Drift</Badge>}
                  </div>
                );
              })
            )}
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Recent Release Events</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="max-h-[340px] overflow-auto space-y-2 pr-1">
              {events.length === 0 ? (
                <p className="text-sm text-muted-foreground">No release events recorded.</p>
              ) : (
                events.map((ev) => (
                  <div key={ev.id} className="rounded-md border p-3">
                    <p className="text-sm font-medium">
                      {ev.channel}: {ev.from_version ?? "none"} → {ev.to_version}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      rollout {ev.rollout_percent}% • {ev.changed_by ?? "system"} • {formatTime(ev.changed_at)}
                    </p>
                  </div>
                ))
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Known Worker Releases</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="max-h-[340px] overflow-auto space-y-2 pr-1">
              {releases.length === 0 ? (
                <p className="text-sm text-muted-foreground">No release records available.</p>
              ) : (
                releases.map((r) => {
                  const artifactCount = Array.isArray(r.artifacts) ? r.artifacts.length : 0;
                  return (
                    <div key={r.version} className="rounded-md border p-3">
                      <p className="text-sm font-medium">{r.version}</p>
                      <p className="text-xs text-muted-foreground">
                        published {formatTime(r.published_at)} • artifacts {artifactCount}
                      </p>
                      {r.notes && (
                        <p className="mt-1 text-xs text-muted-foreground line-clamp-2">{r.notes}</p>
                      )}
                    </div>
                  );
                })
              )}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
