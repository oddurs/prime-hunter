"use client";

import { useEffect, useMemo, useState } from "react";
import { API_BASE, formatTime, relativeTime } from "@/lib/format";
import { ViewHeader } from "@/components/view-header";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

interface LogRow {
  id: number;
  ts: string;
  level: string;
  source: string;
  component: string;
  message: string;
  worker_id?: string | null;
}

const ranges = [
  { label: "1h", hours: 1 },
  { label: "6h", hours: 6 },
  { label: "24h", hours: 24 },
  { label: "7d", hours: 24 * 7 },
];

function toIsoRange(hours: number) {
  const to = new Date();
  const from = new Date(Date.now() - hours * 3600 * 1000);
  return { from: from.toISOString(), to: to.toISOString() };
}

export default function LogsPage() {
  const [range, setRange] = useState(ranges[1]);
  const [level, setLevel] = useState("all");
  const [component, setComponent] = useState("");
  const [worker, setWorker] = useState("");
  const [limit, setLimit] = useState(200);
  const [logs, setLogs] = useState<LogRow[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let active = true;
    async function fetchLogs() {
      const { from, to } = toIsoRange(range.hours);
      const params = new URLSearchParams({ from, to, limit: limit.toString() });
      if (level !== "all") params.set("level", level);
      if (component.trim()) params.set("component", component.trim());
      if (worker.trim()) params.set("worker_id", worker.trim());
      setLoading(true);
      try {
        const res = await fetch(`${API_BASE}/api/observability/logs?${params}`);
        if (!res.ok) throw new Error("fetch failed");
        const data = (await res.json()) as { logs: LogRow[] };
        if (active) setLogs(data.logs ?? []);
      } catch {
        if (active) setLogs([]);
      } finally {
        if (active) setLoading(false);
      }
    }
    fetchLogs();
  }, [range, level, component, worker, limit]);

  const headerMeta = useMemo(() => {
    return `${logs.length} logs · ${range.label} window`;
  }, [logs.length, range.label]);

  return (
    <>
      <ViewHeader
        title="Logs"
        subtitle={headerMeta}
        actions={
          <div className="flex items-center gap-2">
            {ranges.map((r) => (
              <Button
                key={r.label}
                size="sm"
                variant={r.label === range.label ? "default" : "outline"}
                onClick={() => setRange(r)}
              >
                {r.label}
              </Button>
            ))}
          </div>
        }
      />

      <Card className="mb-4">
        <CardHeader className="pb-2">
          <CardTitle className="text-xs font-medium text-muted-foreground">
            Filters
          </CardTitle>
        </CardHeader>
        <CardContent className="grid grid-cols-1 md:grid-cols-4 gap-2">
          <Select value={level} onValueChange={setLevel}>
            <SelectTrigger className="h-8 text-xs">
              <SelectValue placeholder="Level" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All levels</SelectItem>
              <SelectItem value="error">Error</SelectItem>
              <SelectItem value="warn">Warn</SelectItem>
              <SelectItem value="info">Info</SelectItem>
              <SelectItem value="debug">Debug</SelectItem>
            </SelectContent>
          </Select>
          <Input
            value={component}
            onChange={(e) => setComponent(e.target.value)}
            placeholder="Component"
            className="h-8 text-xs"
          />
          <Input
            value={worker}
            onChange={(e) => setWorker(e.target.value)}
            placeholder="Worker ID"
            className="h-8 text-xs"
          />
          <Input
            value={limit.toString()}
            onChange={(e) => setLimit(Number(e.target.value) || 200)}
            placeholder="Limit"
            className="h-8 text-xs"
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-xs font-medium text-muted-foreground">
            Log stream
          </CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Time</TableHead>
                <TableHead>Level</TableHead>
                <TableHead>Component</TableHead>
                <TableHead>Message</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {logs.length === 0 && (
                <TableRow>
                  <TableCell colSpan={4} className="text-xs text-muted-foreground">
                    {loading ? "Loading..." : "No logs in range."}
                  </TableCell>
                </TableRow>
              )}
              {logs.map((log) => (
                <TableRow key={log.id}>
                  <TableCell className="text-xs text-muted-foreground">
                    {formatTime(log.ts)}
                    <div className="text-[11px] text-muted-foreground">
                      {relativeTime(log.ts)}
                    </div>
                  </TableCell>
                  <TableCell className="text-xs uppercase">{log.level}</TableCell>
                  <TableCell className="text-xs">
                    {log.component}
                    {log.worker_id && (
                      <div className="text-[11px] text-muted-foreground">
                        {log.worker_id}
                      </div>
                    )}
                  </TableCell>
                  <TableCell className="text-xs text-muted-foreground">
                    {log.message}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </>
  );
}
