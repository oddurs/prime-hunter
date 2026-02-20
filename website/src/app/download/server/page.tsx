"use client";

import { Section } from "@/components/ui/section";
import { CodeBlock } from "@/components/ui/code-block";
import Link from "next/link";

const buildCommands = `# Install dependencies (Ubuntu/Debian)
sudo apt install build-essential libgmp-dev m4 postgresql
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/darkreach/darkreach.git
cd darkreach
cargo build --release`;

const configCommands = `# Set up PostgreSQL
sudo -u postgres createdb darkreach
export DATABASE_URL="postgres://postgres:password@localhost/darkreach"

# Run the coordinator dashboard
./target/release/darkreach \\
  --database-url "$DATABASE_URL" \\
  --checkpoint /opt/darkreach/darkreach.checkpoint \\
  dashboard --port 7001`;

const systemdUnit = `[Unit]
Description=Darkreach Coordinator (Dashboard)
After=network-online.target
Wants=network-online.target
StartLimitIntervalSec=300
StartLimitBurst=10

[Service]
Type=simple
EnvironmentFile=/opt/darkreach/.env
ExecStart=/usr/local/bin/darkreach \\
  --checkpoint /opt/darkreach/darkreach.checkpoint \\
  dashboard --port 7001
WorkingDirectory=/opt/darkreach
Restart=always
RestartSec=3

ProtectSystem=strict
ReadWritePaths=/opt/darkreach
ProtectHome=true
NoNewPrivileges=true
PrivateTmp=true

LimitNOFILE=65536
MemoryMax=512M

[Install]
WantedBy=multi-user.target`;

const systemdSetup = `# Copy binary
sudo cp target/release/darkreach /usr/local/bin/

# Create working directory
sudo mkdir -p /opt/darkreach
echo 'DATABASE_URL=postgres://...' | sudo tee /opt/darkreach/.env

# Install and start service
sudo cp deploy/darkreach-coordinator.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now darkreach-coordinator`;

const configTable = [
  { flag: "--database-url", env: "DATABASE_URL", desc: "PostgreSQL connection string" },
  { flag: "--checkpoint", env: "—", desc: "Checkpoint file path (default: darkreach.checkpoint)" },
  { flag: "--port", env: "—", desc: "Dashboard HTTP port (default: 7001)" },
  { flag: "--qos", env: "—", desc: "Set macOS QoS_CLASS_USER_INITIATED for Rayon threads" },
];

export default function ServerPage() {
  return (
    <>
      <Section>
        <nav className="text-sm text-text-muted mb-8">
          <Link href="/download" className="hover:text-text transition-colors">
            Download
          </Link>
          <span className="mx-2">/</span>
          <span className="text-text">Coordinator Setup</span>
        </nav>

        <h1 className="text-4xl font-bold text-text mb-4">
          Coordinator Setup
        </h1>
        <p className="text-text-muted max-w-3xl mb-12">
          The coordinator runs the darkreach dashboard — an Axum web server that
          provides the REST API, WebSocket coordination, fleet management, and
          the real-time UI. Workers connect to it to receive work and report
          results.
        </p>

        <div className="space-y-12">
          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              Prerequisites
            </h2>
            <ul className="list-disc list-inside text-text-muted space-y-1">
              <li>Linux server (Ubuntu 22.04+ recommended)</li>
              <li>Rust toolchain (1.75+)</li>
              <li>GMP library (libgmp-dev)</li>
              <li>PostgreSQL 14+</li>
            </ul>
          </div>

          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              1. Build from source
            </h2>
            <CodeBlock language="bash">{buildCommands}</CodeBlock>
          </div>

          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              2. Configure and run
            </h2>
            <CodeBlock language="bash">{configCommands}</CodeBlock>
            <p className="text-sm text-text-muted mt-3">
              The dashboard will be available at{" "}
              <code className="text-accent-purple">http://your-server:7001</code>.
            </p>
          </div>

          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              3. Systemd service
            </h2>
            <p className="text-text-muted mb-4">
              For production, run the coordinator as a systemd service with
              automatic restarts and security hardening.
            </p>
            <CodeBlock language="ini">{systemdUnit}</CodeBlock>
            <p className="text-sm text-text-muted mt-4 mb-4">
              Install and enable the service:
            </p>
            <CodeBlock language="bash">{systemdSetup}</CodeBlock>
          </div>

          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              Configuration Reference
            </h2>
            <div className="overflow-x-auto rounded-lg border border-border">
              <table className="w-full text-sm">
                <thead>
                  <tr className="bg-bg-secondary text-text-muted text-left">
                    <th className="px-4 py-3 font-medium">Flag</th>
                    <th className="px-4 py-3 font-medium">Env Var</th>
                    <th className="px-4 py-3 font-medium">Description</th>
                  </tr>
                </thead>
                <tbody>
                  {configTable.map((row) => (
                    <tr key={row.flag} className="border-t border-border">
                      <td className="px-4 py-3 font-mono text-accent-purple text-xs">
                        {row.flag}
                      </td>
                      <td className="px-4 py-3 font-mono text-text-muted text-xs">
                        {row.env}
                      </td>
                      <td className="px-4 py-3 text-text-muted">{row.desc}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </Section>
    </>
  );
}
