import { Brain, Server, ShieldCheck } from "lucide-react";
import { Section } from "./ui/section";
import { Card } from "./ui/card";

const features = [
  {
    icon: Brain,
    title: "Self-Optimizing Engine",
    description:
      "AI agents research strategies, tune sieve depths, and select optimal algorithms for each prime form. The system learns which approaches yield results fastest.",
  },
  {
    icon: Server,
    title: "Autonomous Orchestration",
    description:
      "Agents manage campaigns, schedule searches across workers, and dynamically allocate fleet resources. No manual intervention required.",
  },
  {
    icon: ShieldCheck,
    title: "Provable Results",
    description:
      "Deterministic primality certificates — Pocklington, Morrison, BLS — with independently verifiable witness data. Every discovery is mathematically proven.",
  },
];

export function FeatureGrid() {
  return (
    <Section>
      <h2 className="text-3xl font-bold text-text mb-4 text-center">
        AI-Powered Discovery
      </h2>
      <p className="text-text-muted text-center max-w-2xl mx-auto mb-12">
        darkreach combines autonomous AI agents with high-performance number
        theory algorithms to push the boundaries of mathematical discovery.
      </p>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {features.map((feature) => (
          <Card key={feature.title} hover>
            <div className="inline-flex items-center justify-center w-12 h-12 rounded-lg bg-bg border border-border text-accent-purple mb-4">
              <feature.icon size={24} />
            </div>
            <h3 className="text-lg font-semibold text-text mb-2">
              {feature.title}
            </h3>
            <p className="text-sm text-text-muted leading-relaxed">
              {feature.description}
            </p>
          </Card>
        ))}
      </div>
    </Section>
  );
}
