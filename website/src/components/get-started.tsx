import { UserPlus, Cpu, BarChart3 } from "lucide-react";
import { Section } from "./ui/section";
import Link from "next/link";

const steps = [
  {
    icon: UserPlus,
    title: "Register",
    description:
      "Register as an operator and receive your API key. Requires Rust and GMP to build.",
    code: "darkreach register",
  },
  {
    icon: Cpu,
    title: "Connect",
    description:
      "Connect your node to the network. Work is assigned automatically and results are proven and logged.",
    code: "darkreach run",
  },
  {
    icon: BarChart3,
    title: "Monitor",
    description:
      "Monitor your nodes, track discoveries, earn compute credits, and climb the operator leaderboard.",
    code: "app.darkreach.ai",
  },
];

export function GetStarted() {
  return (
    <Section>
      <h2 className="text-3xl font-bold text-foreground mb-4 text-center">
        Become an Operator in Three Steps
      </h2>
      <p className="text-muted-foreground text-center max-w-2xl mx-auto mb-12">
        Register, connect your nodes, and start earning compute credits.
      </p>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
        {steps.map((step, i) => (
          <div key={step.title} className="text-center">
            <div className="relative inline-flex items-center justify-center w-16 h-16 rounded-full bg-card border border-border text-accent-purple mb-4">
              <step.icon size={28} />
              <span className="absolute -top-1 -right-1 w-6 h-6 rounded-full bg-accent-purple text-white text-xs font-bold flex items-center justify-center">
                {i + 1}
              </span>
            </div>
            <h3 className="text-lg font-semibold text-foreground mb-2">
              {step.title}
            </h3>
            <p className="text-sm text-muted-foreground leading-relaxed mb-4">
              {step.description}
            </p>
            <div className="inline-block rounded-md bg-background border border-border px-4 py-2 font-mono text-sm text-accent-green">
              {step.code}
            </div>
          </div>
        ))}
      </div>

      <div className="flex flex-col sm:flex-row items-center justify-center gap-4 mt-12">
        <Link
          href="/download"
          className="inline-flex items-center px-6 py-3 rounded-md bg-accent-purple text-white font-medium hover:opacity-90 transition-opacity"
        >
          Full Setup Guide
        </Link>
        <Link
          href="/docs/getting-started"
          className="inline-flex items-center px-6 py-3 rounded-md border border-border text-muted-foreground font-medium hover:text-foreground transition-colors"
        >
          Read the Docs
        </Link>
      </div>
    </Section>
  );
}
