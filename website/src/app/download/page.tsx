"use client";

import { Section } from "@/components/ui/section";
import { InstallCommand } from "@/components/install-command";
import { Card } from "@/components/ui/card";
import { systemRequirements } from "@/lib/install-commands";
import { Server, Monitor, ArrowRight } from "lucide-react";
import Link from "next/link";

export default function DownloadPage() {
  return (
    <>
      <Section>
        <h1 className="text-4xl font-bold text-text mb-4">Download darkreach</h1>
        <p className="text-text-muted max-w-2xl mb-10">
          Install darkreach and start hunting primes. Detected your OS
          automatically — select a different platform below if needed.
        </p>

        <InstallCommand />
      </Section>

      <Section secondary>
        <h2 className="text-2xl font-bold text-text mb-8">
          System Requirements
        </h2>

        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-bg text-text-muted text-left">
                <th className="px-4 py-3 font-medium">Component</th>
                <th className="px-4 py-3 font-medium">Minimum</th>
                <th className="px-4 py-3 font-medium">Recommended</th>
              </tr>
            </thead>
            <tbody>
              {systemRequirements.map((req) => (
                <tr key={req.component} className="border-t border-border">
                  <td className="px-4 py-3 text-text font-medium">
                    {req.component}
                  </td>
                  <td className="px-4 py-3 text-text-muted">{req.minimum}</td>
                  <td className="px-4 py-3 text-text-muted">
                    {req.recommended}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Section>

      <Section>
        <h2 className="text-2xl font-bold text-text mb-8">Deployment Guides</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <Link href="/download/server">
            <Card hover className="group cursor-pointer">
              <div className="flex items-center gap-3 mb-3">
                <div className="w-10 h-10 rounded-lg bg-accent-purple/10 border border-accent-purple/30 flex items-center justify-center text-accent-purple">
                  <Server size={20} />
                </div>
                <h3 className="text-lg font-semibold text-text">
                  Coordinator Setup
                </h3>
                <ArrowRight
                  size={16}
                  className="ml-auto text-text-muted group-hover:text-accent-purple transition-colors"
                />
              </div>
              <p className="text-sm text-text-muted">
                Deploy a self-hosted coordinator with PostgreSQL, systemd
                services, and the real-time dashboard.
              </p>
            </Card>
          </Link>

          <Link href="/download/worker">
            <Card hover className="group cursor-pointer">
              <div className="flex items-center gap-3 mb-3">
                <div className="w-10 h-10 rounded-lg bg-accent-green/10 border border-accent-green/30 flex items-center justify-center text-accent-green">
                  <Monitor size={20} />
                </div>
                <h3 className="text-lg font-semibold text-text">
                  Worker Deployment
                </h3>
                <ArrowRight
                  size={16}
                  className="ml-auto text-text-muted group-hover:text-accent-green transition-colors"
                />
              </div>
              <p className="text-sm text-text-muted">
                Connect worker nodes to a coordinator and contribute compute to
                the prime search network.
              </p>
            </Card>
          </Link>
        </div>
      </Section>
    </>
  );
}
