"use client";

import { Section } from "./ui/section";
import { CodeBlock } from "./ui/code-block";
import { Server, Monitor } from "lucide-react";
import Link from "next/link";

const workerCode = `# Install and run a worker
git clone https://github.com/darkreach/darkreach.git
cd darkreach && cargo build --release
./target/release/darkreach \\
  --coordinator http://COORDINATOR:7001 \\
  --worker-id my-worker \\
  kbn --k 3 --base 2 --min-n 100000 --max-n 500000`;

const serverCode = `# Self-host a coordinator
git clone https://github.com/darkreach/darkreach.git
cd darkreach && cargo build --release
export DATABASE_URL="postgres://..."
./target/release/darkreach dashboard --port 7001`;

export function CtaSection() {
  return (
    <Section>
      <h2 className="text-3xl font-bold text-text mb-4 text-center">
        Start Contributing
      </h2>
      <p className="text-text-muted text-center max-w-2xl mx-auto mb-12">
        Join the search as a volunteer worker, or deploy your own coordinator.
      </p>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
        <div className="rounded-lg border border-border bg-bg-secondary p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-10 h-10 rounded-lg bg-accent-green/10 border border-accent-green/30 flex items-center justify-center text-accent-green">
              <Monitor size={20} />
            </div>
            <div>
              <h3 className="text-lg font-semibold text-text">Run a Worker</h3>
              <p className="text-sm text-text-muted">
                Contribute compute to the network
              </p>
            </div>
          </div>
          <CodeBlock>{workerCode}</CodeBlock>
          <Link
            href="/download/worker"
            className="inline-flex items-center mt-4 text-sm text-primary hover:underline"
          >
            Full worker guide →
          </Link>
        </div>

        <div className="rounded-lg border border-border bg-bg-secondary p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-10 h-10 rounded-lg bg-accent-purple/10 border border-accent-purple/30 flex items-center justify-center text-accent-purple">
              <Server size={20} />
            </div>
            <div>
              <h3 className="text-lg font-semibold text-text">Self-Host</h3>
              <p className="text-sm text-text-muted">
                Deploy your own coordinator
              </p>
            </div>
          </div>
          <CodeBlock>{serverCode}</CodeBlock>
          <Link
            href="/download/server"
            className="inline-flex items-center mt-4 text-sm text-primary hover:underline"
          >
            Full coordinator guide →
          </Link>
        </div>
      </div>
    </Section>
  );
}
