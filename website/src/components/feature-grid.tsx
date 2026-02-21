import { Brain, Server, Zap, Target, BarChart3, UserCheck } from "lucide-react";
import { Section } from "./ui/section";

const features = [
  {
    icon: Brain,
    title: "Strategy Engine",
    description:
      "AI agents score forms by record gap, yield, and cost efficiency. Auto-create projects and optimize sieve depths for maximum discovery rate.",
    accent: "from-indigo-500 to-violet-500",
  },
  {
    icon: Server,
    title: "Network Orchestration",
    description:
      "Service and compute nodes register, claim work blocks via PostgreSQL, and coordinate results across the deployment lifecycle. No manual intervention required.",
    accent: "from-emerald-500 to-teal-500",
  },
  {
    icon: UserCheck,
    title: "Operator System",
    description:
      "Register nodes, manage API keys, earn compute credits. Role-based access with trust levels for teams and researchers.",
    accent: "from-amber-500 to-orange-500",
  },
  {
    icon: Zap,
    title: "12 Prime Forms",
    description:
      "Factorial, palindromic, k\u00b7b^n\u00b11, twin, Wagstaff, generalized Fermat, and six more. Each with a form-specific sieve and test.",
    accent: "from-violet-500 to-purple-500",
  },
  {
    icon: Target,
    title: "Deep Sieve Pipeline",
    description:
      "Wheel factorization, BSGS, Montgomery multiplication, and Pollard P-1 filtering eliminate composites before expensive tests.",
    accent: "from-cyan-500 to-blue-500",
  },
  {
    icon: BarChart3,
    title: "Real-Time Dashboard",
    description:
      "Live discovery feed, network health, search management, leaderboard, and performance charts. Know exactly what your network is doing.",
    accent: "from-rose-500 to-pink-500",
  },
];

export function FeatureGrid() {
  return (
    <Section>
      <div className="text-center mb-16">
        <p className="text-sm font-medium text-accent-purple uppercase tracking-wider mb-3">
          Capabilities
        </p>
        <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
          Everything you need to hunt primes
        </h2>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          From sieving candidates to generating mathematical proofs, darkreach
          handles the full discovery pipeline.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
        {features.map((feature) => (
          <div
            key={feature.title}
            className="group relative rounded-xl border border-border bg-card p-6 transition-all hover:border-border/80 hover:shadow-lg hover:shadow-accent-purple/5"
          >
            <div className="absolute inset-x-0 top-0 h-px">
              <div className={`h-full w-0 group-hover:w-full bg-gradient-to-r ${feature.accent} transition-all duration-500 rounded-t-xl`} />
            </div>
            <div className={`inline-flex items-center justify-center w-10 h-10 rounded-lg bg-gradient-to-br ${feature.accent} text-white mb-4`}>
              <feature.icon size={20} />
            </div>
            <h3 className="text-base font-semibold text-foreground mb-2">
              {feature.title}
            </h3>
            <p className="text-sm text-muted-foreground leading-relaxed">
              {feature.description}
            </p>
          </div>
        ))}
      </div>
    </Section>
  );
}
