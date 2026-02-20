"use client";

import { Section } from "@/components/ui/section";
import { StatusCard } from "@/components/status-card";
import { UptimeBar } from "@/components/uptime-bar";
import { Badge } from "@/components/ui/badge";
import { services, fleetStats, recentIncidents } from "@/lib/status-data";

export default function StatusPage() {
  const allOperational = services.every((s) => s.status === "operational");

  return (
    <>
      <Section>
        <div className="flex items-center gap-4 mb-8">
          <h1 className="text-4xl font-bold text-foreground">System Status</h1>
          {allOperational ? (
            <Badge variant="green">All Systems Operational</Badge>
          ) : (
            <Badge variant="orange">Partial Outage</Badge>
          )}
        </div>

        <div className="space-y-3">
          {services.map((service) => (
            <StatusCard key={service.name} service={service} />
          ))}
        </div>
      </Section>

      <Section secondary>
        <h2 className="text-2xl font-bold text-foreground mb-8">Fleet Overview</h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-6 mb-12">
          <div className="text-center">
            <div className="text-3xl font-bold font-mono text-foreground">
              {fleetStats.activeWorkers}
            </div>
            <div className="text-sm text-muted-foreground">Active Workers</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold font-mono text-foreground">
              {fleetStats.totalCores}
            </div>
            <div className="text-sm text-muted-foreground">Total Cores</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold font-mono text-accent-green">
              {fleetStats.uptimePercent}%
            </div>
            <div className="text-sm text-muted-foreground">Uptime (30d)</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold font-mono text-foreground">
              {fleetStats.primesLast24h}
            </div>
            <div className="text-sm text-muted-foreground">Primes (24h)</div>
          </div>
        </div>

        <div className="space-y-8">
          <UptimeBar label="Coordinator (api.darkreach.ai)" />
          <UptimeBar label="Dashboard (app.darkreach.ai)" />
          <UptimeBar label="Database (Supabase)" />
          <UptimeBar label="Website (darkreach.ai)" />
        </div>
      </Section>

      <Section>
        <h2 className="text-2xl font-bold text-foreground mb-8">Recent Incidents</h2>
        {recentIncidents.length === 0 ? (
          <p className="text-muted-foreground">No recent incidents.</p>
        ) : (
          <div className="space-y-4">
            {recentIncidents.map((incident) => (
              <div
                key={incident.date}
                className="border border-border rounded-md p-4 bg-card"
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-3">
                    <h3 className="text-foreground font-semibold">
                      {incident.title}
                    </h3>
                    <Badge
                      variant={
                        incident.status === "resolved" ? "green" : "orange"
                      }
                    >
                      {incident.status}
                    </Badge>
                  </div>
                  <span className="text-sm text-muted-foreground">
                    {incident.date}
                  </span>
                </div>
                <p className="text-sm text-muted-foreground">
                  {incident.description}
                </p>
                <p className="text-xs text-muted-foreground mt-1">
                  Duration: {incident.duration}
                </p>
              </div>
            ))}
          </div>
        )}
      </Section>
    </>
  );
}
