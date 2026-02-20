"use client";

import { Section } from "@/components/ui/section";
import { CodeBlock } from "@/components/ui/code-block";
import Link from "next/link";

const buildCommands = `# Install dependencies
# macOS: brew install gmp rust
# Linux: sudo apt install build-essential libgmp-dev m4

git clone https://github.com/darkreach/darkreach.git
cd darkreach
cargo build --release`;

const runWorker = `# Connect to a coordinator and start searching
./target/release/darkreach \\
  --coordinator http://COORDINATOR:7001 \\
  --worker-id my-worker \\
  --checkpoint darkreach.checkpoint \\
  kbn --k 3 --base 2 --min-n 100000 --max-n 500000`;

const systemdTemplate = `[Unit]
Description=Darkreach Worker (%i)
After=network.target

[Service]
Type=simple
EnvironmentFile=/opt/darkreach/.env
ExecStart=/usr/local/bin/darkreach \\
  --coordinator http://COORDINATOR:7001 \\
  --worker-id %H-%i \\
  --checkpoint /opt/darkreach/darkreach-%i.checkpoint \\
  kbn --k 3 --base 2 --min-n 1000 --max-n 100000
WorkingDirectory=/opt/darkreach
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target`;

const scalingCommands = `# Install worker template
sudo cp deploy/darkreach-worker@.service /etc/systemd/system/
sudo systemctl daemon-reload

# Start 4 worker instances
sudo systemctl enable --now darkreach-worker@{1..4}

# Check status
sudo systemctl status 'darkreach-worker@*'`;

export default function WorkerPage() {
  return (
    <>
      <Section>
        <nav className="text-sm text-text-muted mb-8">
          <Link href="/download" className="hover:text-text transition-colors">
            Download
          </Link>
          <span className="mx-2">/</span>
          <span className="text-text">Worker Deployment</span>
        </nav>

        <h1 className="text-4xl font-bold text-text mb-4">
          Worker Deployment
        </h1>
        <p className="text-text-muted max-w-3xl mb-12">
          Workers are the compute nodes that run prime searches. They connect to
          a coordinator, claim work blocks, run sieves and primality tests, and
          report results. Each worker can handle any of the 12 prime forms.
        </p>

        <div className="space-y-12">
          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              Prerequisites
            </h2>
            <ul className="list-disc list-inside text-text-muted space-y-1">
              <li>Rust toolchain (1.75+)</li>
              <li>GMP library</li>
              <li>Network access to the coordinator</li>
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
              2. Run a worker
            </h2>
            <p className="text-text-muted mb-4">
              Point the worker at your coordinator and specify a search form with
              its parameters.
            </p>
            <CodeBlock language="bash">{runWorker}</CodeBlock>
            <p className="text-sm text-text-muted mt-3">
              The worker will register with the coordinator, appear in the fleet
              dashboard, and start claiming work blocks automatically.
            </p>
          </div>

          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              3. Verify heartbeat
            </h2>
            <p className="text-text-muted">
              Once the worker is running, verify it appears in the coordinator
              dashboard under the Fleet tab. Workers send heartbeats every 30
              seconds. You can also check the coordinator logs for registration
              events.
            </p>
          </div>

          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              4. Scaling with systemd
            </h2>
            <p className="text-text-muted mb-4">
              Use systemd template units to run multiple worker instances on the
              same machine. Each instance gets its own checkpoint file and worker
              ID.
            </p>
            <CodeBlock language="ini">{systemdTemplate}</CodeBlock>
            <p className="text-sm text-text-muted mt-4 mb-4">
              Deploy and scale:
            </p>
            <CodeBlock language="bash">{scalingCommands}</CodeBlock>
          </div>

          <div>
            <h2 className="text-2xl font-semibold text-text mb-2">
              Monitoring
            </h2>
            <p className="text-text-muted">
              Worker status, throughput, and prime discoveries are visible in the{" "}
              <a
                href="https://app.darkreach.ai"
                className="text-primary hover:underline"
              >
                dashboard
              </a>
              . The Fleet tab shows all connected workers with their current
              search form, candidates tested, and last heartbeat time.
            </p>
          </div>
        </div>
      </Section>
    </>
  );
}
