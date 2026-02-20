export type ServiceStatus = "operational" | "degraded" | "down";

export interface Service {
  name: string;
  status: ServiceStatus;
  url: string;
  latency?: string;
  description: string;
}

export const services: Service[] = [
  {
    name: "Coordinator",
    status: "operational",
    url: "api.darkreach.ai",
    latency: "12ms",
    description: "REST API and WebSocket coordination server",
  },
  {
    name: "Dashboard",
    status: "operational",
    url: "app.darkreach.ai",
    latency: "45ms",
    description: "Real-time monitoring and search management UI",
  },
  {
    name: "Database",
    status: "operational",
    url: "Supabase (eu-central-1)",
    latency: "8ms",
    description: "PostgreSQL storage for primes, workers, and jobs",
  },
  {
    name: "Website",
    status: "operational",
    url: "darkreach.ai",
    latency: "23ms",
    description: "Landing page, docs, and status page",
  },
];

export interface FleetStats {
  activeWorkers: number;
  totalCores: number;
  uptimePercent: number;
  primesLast24h: number;
}

export const fleetStats: FleetStats = {
  activeWorkers: 38,
  totalCores: 152,
  uptimePercent: 99.94,
  primesLast24h: 12,
};

export interface UptimeDay {
  date: string;
  status: ServiceStatus;
}

export function generateUptimeDays(days: number): UptimeDay[] {
  const result: UptimeDay[] = [];
  const now = new Date();
  for (let i = days - 1; i >= 0; i--) {
    const date = new Date(now);
    date.setDate(date.getDate() - i);
    const rand = Math.random();
    let status: ServiceStatus = "operational";
    if (rand < 0.02) status = "down";
    else if (rand < 0.06) status = "degraded";
    result.push({ date: date.toISOString().split("T")[0], status });
  }
  return result;
}

export interface Incident {
  date: string;
  title: string;
  description: string;
  status: "resolved" | "monitoring";
  duration: string;
}

export const recentIncidents: Incident[] = [
  {
    date: "2026-02-18",
    title: "Database connection pool exhaustion",
    description:
      "Supabase session pooler hit connection limit. Reduced pool size from 5 to 2.",
    status: "resolved",
    duration: "23 minutes",
  },
  {
    date: "2026-02-10",
    title: "Worker heartbeat delays",
    description:
      "Network congestion caused heartbeat timeouts for 3 workers. Workers auto-reconnected.",
    status: "resolved",
    duration: "8 minutes",
  },
  {
    date: "2026-01-28",
    title: "Coordinator restart",
    description:
      "Scheduled maintenance for darkreach binary update. Zero downtime with rolling restart.",
    status: "resolved",
    duration: "< 1 minute",
  },
];
