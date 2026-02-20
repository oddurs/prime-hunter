import { Filter, Cpu, ShieldCheck, Network } from "lucide-react";
import { Section } from "./ui/section";

const steps = [
  {
    icon: Filter,
    title: "Sieve",
    description:
      "Eliminate composites with form-specific sieves — wheel factorization, BSGS, and Pollard P-1 filtering.",
  },
  {
    icon: Cpu,
    title: "Test",
    description:
      "Miller-Rabin pre-screening, then form-specific tests — Proth, LLR, Pepin — accelerated by PFGW.",
  },
  {
    icon: ShieldCheck,
    title: "Prove",
    description:
      "Generate deterministic primality certificates with independently verifiable witness data.",
  },
  {
    icon: Network,
    title: "Coordinate",
    description:
      "Distribute work across the fleet, collect results, and publish discoveries to the global database.",
  },
];

export function Pipeline() {
  return (
    <Section secondary>
      <h2 className="text-3xl font-bold text-text mb-4 text-center">
        Discovery Pipeline
      </h2>
      <p className="text-text-muted text-center max-w-2xl mx-auto mb-12">
        Every candidate passes through a four-stage pipeline optimized for each
        prime form.
      </p>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
        {steps.map((step, i) => (
          <div key={step.title} className="text-center">
            <div className="relative inline-flex items-center justify-center w-16 h-16 rounded-full bg-bg border border-border text-accent-purple mb-4">
              <step.icon size={28} />
              <span className="absolute -top-1 -right-1 w-6 h-6 rounded-full bg-accent-purple text-white text-xs font-bold flex items-center justify-center">
                {i + 1}
              </span>
            </div>
            <h3 className="text-lg font-semibold text-text mb-2">
              {step.title}
            </h3>
            <p className="text-sm text-text-muted leading-relaxed max-w-xs mx-auto">
              {step.description}
            </p>
          </div>
        ))}
      </div>
    </Section>
  );
}
