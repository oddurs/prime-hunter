"use client";

import { useEffect, useState } from "react";

const API_BASE = "https://api.darkreach.ai";

interface Stats {
  primes_found: string;
  candidates_tested: string;
  active_nodes: string;
  compute_hours: string;
  search_forms: string;
}

const FALLBACK: Stats = {
  primes_found: "392,009",
  candidates_tested: "14.2B",
  active_nodes: "4",
  compute_hours: "127K",
  search_forms: "12",
};

function formatNumber(n: number): string {
  if (n >= 1_000_000_000) return (n / 1_000_000_000).toFixed(1) + "B";
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return n.toLocaleString("en-US");
  return String(n);
}

export function StatsBar() {
  const [stats, setStats] = useState(FALLBACK);
  const [live, setLive] = useState(false);

  useEffect(() => {
    let active = true;
    async function fetchStats() {
      try {
        const [statusRes, networkRes] = await Promise.all([
          fetch(`${API_BASE}/api/status`),
          fetch(`${API_BASE}/api/fleet`),
        ]);

        if (!statusRes.ok || !networkRes.ok) return;

        const status = (await statusRes.json()) as {
          total_primes?: number;
          total_tested?: number;
          uptime_secs?: number;
        };
        const network = (await networkRes.json()) as {
          workers?: Array<{ status?: string }>;
        };

        if (!active) return;

        const nodes = network.workers ?? [];
        const activeCount = nodes.filter(
          (n) => n.status === "active" || n.status === "running"
        ).length;

        const hours = status.uptime_secs
          ? Math.round(((status.uptime_secs ?? 0) * (activeCount || 1)) / 3600)
          : 0;

        setStats({
          primes_found: formatNumber(status.total_primes ?? 0),
          candidates_tested: formatNumber(status.total_tested ?? 0),
          active_nodes: String(activeCount || nodes.length),
          compute_hours: hours > 0 ? formatNumber(hours) : FALLBACK.compute_hours,
          search_forms: "12",
        });
        setLive(true);
      } catch {
        // Keep fallback values
      }
    }

    fetchStats();
    const timer = setInterval(fetchStats, 30000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, []);

  const items = [
    { label: "Primes Found", value: stats.primes_found, isLive: true },
    { label: "Candidates Tested", value: stats.candidates_tested, isLive: true },
    { label: "Active Nodes", value: stats.active_nodes, isLive: true },
    { label: "Compute Hours", value: stats.compute_hours, isLive: true },
    { label: "Search Forms", value: stats.search_forms, isLive: false },
  ];

  return (
    <section className="relative border-y border-border/50">
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-accent-purple/30 to-transparent" />
      <div className="mx-auto max-w-7xl px-6 sm:px-8 lg:px-12 py-8">
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-6">
          {items.map((stat) => (
            <div key={stat.label} className="text-center">
              <div className="flex items-center justify-center gap-2 mb-1">
                {stat.isLive && (
                  <span
                    className={`inline-block w-1.5 h-1.5 rounded-full ${
                      live ? "bg-accent-green pulse-green" : "bg-muted-foreground/40"
                    }`}
                  />
                )}
                <span className="text-3xl font-bold font-mono tracking-tight text-foreground">
                  {stat.value}
                </span>
              </div>
              <div className="text-sm text-muted-foreground">{stat.label}</div>
            </div>
          ))}
        </div>
      </div>
      <div className="absolute inset-x-0 bottom-0 h-px bg-gradient-to-r from-transparent via-accent-purple/30 to-transparent" />
    </section>
  );
}
