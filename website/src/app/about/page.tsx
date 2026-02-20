import { Section } from "@/components/ui/section";
import { Card } from "@/components/ui/card";
import { Timeline } from "@/components/timeline";
import { Github } from "lucide-react";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "About",
  description: "The mission, timeline, and technology behind darkreach.",
};

const techStack = [
  {
    name: "Rust",
    description:
      "Zero-cost abstractions, memory safety, and fearless concurrency for the engine and server.",
  },
  {
    name: "GMP (rug)",
    description:
      "GNU Multiple Precision Arithmetic — the gold standard for arbitrary-precision integer math.",
  },
  {
    name: "PFGW / GWNUM",
    description:
      "Specialized number theory software for 50-100x acceleration on large primality tests.",
  },
  {
    name: "PostgreSQL",
    description:
      "Relational database for primes, workers, jobs, and work distribution with row-level locking.",
  },
  {
    name: "Axum",
    description:
      "Async Rust web framework for the coordinator REST API and WebSocket server.",
  },
  {
    name: "Next.js",
    description:
      "React framework for the dashboard (app.darkreach.ai) and website (darkreach.ai).",
  },
];

export default function AboutPage() {
  return (
    <>
      <Section>
        <h1 className="text-4xl font-bold text-foreground mb-6">About darkreach</h1>
        <div className="max-w-3xl space-y-4 text-muted-foreground">
          <p>
            darkreach is an AI-driven distributed computing platform for
            scientific discovery. It combines autonomous AI agents with
            high-performance algorithms to research, optimize, and execute
            computational campaigns across a fleet of servers.
          </p>
          <p>
            Our current focus is prime number discovery — searching for 12
            special forms of prime numbers with deterministic proofs. But the
            architecture is general: the same agent-driven orchestration can
            tackle any embarrassingly parallel scientific computation.
          </p>
          <p>
            The project is fully open source under the MIT license. We believe
            mathematical discoveries should be independently verifiable, the
            tools should be available to everyone, and the code should serve as a
            teaching resource for computational number theory.
          </p>
        </div>
      </Section>

      <Section secondary>
        <h2 className="text-2xl font-bold text-foreground mb-8">Project Timeline</h2>
        <div className="max-w-2xl">
          <Timeline />
        </div>
      </Section>

      <Section>
        <h2 className="text-2xl font-bold text-foreground mb-8">Tech Stack</h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {techStack.map((tech) => (
            <Card key={tech.name}>
              <h3 className="text-foreground font-semibold mb-2">{tech.name}</h3>
              <p className="text-sm text-muted-foreground">{tech.description}</p>
            </Card>
          ))}
        </div>
      </Section>

      <Section secondary>
        <div className="text-center max-w-2xl mx-auto">
          <h2 className="text-2xl font-bold text-foreground mb-4">Open Source</h2>
          <p className="text-muted-foreground mb-6">
            darkreach is licensed under MIT. Every line of engine code, every
            proof algorithm, and every orchestration strategy is open for
            inspection, modification, and contribution.
          </p>
          <a
            href="https://github.com/darkreach/darkreach"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-6 py-3 rounded-md border border-border text-foreground hover:border-text-muted transition-colors"
          >
            <Github size={20} />
            View on GitHub
          </a>
        </div>
      </Section>
    </>
  );
}
