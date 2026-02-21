import { Filter, Cpu, ShieldCheck, Network } from "lucide-react";
import { Section } from "./ui/section";

const steps = [
  {
    icon: Filter,
    title: "Sieve",
    stat: "99.9%",
    statLabel: "ELIMINATED",
    description:
      "Wheel factorization, BSGS, and Pollard P-1 filtering remove composites before testing.",
    accent: "from-indigo-500 to-violet-500",
  },
  {
    icon: Cpu,
    title: "Test",
    stat: "25+",
    statLabel: "MR ROUNDS",
    description:
      "Proth, LLR, Pépin, and Frobenius tests accelerated by PFGW for large candidates.",
    accent: "from-violet-500 to-purple-500",
  },
  {
    icon: ShieldCheck,
    title: "Prove",
    stat: "100%",
    statLabel: "DETERMINISTIC",
    description:
      "Pocklington, Morrison, and BLS certificates with full witness data. Verifiable.",
    accent: "from-emerald-500 to-teal-500",
  },
  {
    icon: Network,
    title: "Coordinate",
    stat: "< 1s",
    statLabel: "BLOCK CLAIM",
    description:
      "Operator nodes claim work blocks via PostgreSQL. Results verified through trust-based quorum with adaptive replication.",
    accent: "from-amber-500 to-orange-500",
  },
];

export function Pipeline() {
  return (
    <Section secondary>
      <div className="text-center mb-16">
        <p className="text-sm font-medium text-accent-purple uppercase tracking-wider mb-3">
          How it works
        </p>
        <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
          Discovery Pipeline
        </h2>
        <p className="text-lg text-muted-foreground max-w-2xl mx-auto">
          Every candidate passes through four stages. Only proven primes survive.
        </p>
      </div>

      {/* Desktop: single pipeline container */}
      <div className="hidden lg:block max-w-5xl mx-auto">
        <div className="rounded-2xl border border-border bg-background overflow-hidden">
          {/* Progress track */}
          <div className="relative h-1.5 bg-border/50">
            <div className="absolute inset-y-0 left-0 right-0 bg-gradient-to-r from-indigo-500 via-violet-500 via-emerald-500 to-amber-500 opacity-60" />
          </div>

          {/* Stages */}
          <div className="grid grid-cols-4 divide-x divide-border">
            {steps.map((step, i) => (
              <div key={step.title} className="p-7">
                {/* Number + icon */}
                <div className="flex items-center justify-between mb-6">
                  <span className="text-[40px] font-bold font-mono leading-none text-muted-foreground/15 select-none">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <div className={`w-9 h-9 rounded-lg bg-gradient-to-br ${step.accent} flex items-center justify-center text-white`}>
                    <step.icon size={18} />
                  </div>
                </div>

                {/* Title */}
                <h3 className="text-lg font-semibold text-foreground tracking-tight mb-4">
                  {step.title}
                </h3>

                {/* Stat */}
                <div className="mb-1">
                  <span className="text-[32px] font-bold font-mono leading-none gradient-text">
                    {step.stat}
                  </span>
                </div>
                <p className="text-[11px] font-medium text-muted-foreground/60 tracking-widest mb-4">
                  {step.statLabel}
                </p>

                {/* Description */}
                <p className="text-[13px] text-muted-foreground leading-relaxed">
                  {step.description}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Tablet: 2-column pipeline */}
      <div className="hidden sm:block lg:hidden">
        <div className="rounded-2xl border border-border bg-background overflow-hidden">
          <div className="relative h-1 bg-border/50">
            <div className="absolute inset-y-0 left-0 right-0 bg-gradient-to-r from-indigo-500 via-violet-500 via-emerald-500 to-amber-500 opacity-60" />
          </div>
          <div className="grid grid-cols-2 divide-x divide-border">
            {steps.slice(0, 2).map((step, i) => (
              <div key={step.title} className="p-6">
                <div className="flex items-center justify-between mb-4">
                  <span className="text-[32px] font-bold font-mono leading-none text-muted-foreground/15 select-none">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <div className={`w-8 h-8 rounded-lg bg-gradient-to-br ${step.accent} flex items-center justify-center text-white`}>
                    <step.icon size={16} />
                  </div>
                </div>
                <h3 className="text-base font-semibold text-foreground tracking-tight mb-3">
                  {step.title}
                </h3>
                <div className="mb-1">
                  <span className="text-2xl font-bold font-mono gradient-text">{step.stat}</span>
                </div>
                <p className="text-[10px] font-medium text-muted-foreground/60 tracking-widest mb-3">
                  {step.statLabel}
                </p>
                <p className="text-[13px] text-muted-foreground leading-relaxed">
                  {step.description}
                </p>
              </div>
            ))}
          </div>
          <div className="border-t border-border grid grid-cols-2 divide-x divide-border">
            {steps.slice(2).map((step, i) => (
              <div key={step.title} className="p-6">
                <div className="flex items-center justify-between mb-4">
                  <span className="text-[32px] font-bold font-mono leading-none text-muted-foreground/15 select-none">
                    {String(i + 3).padStart(2, "0")}
                  </span>
                  <div className={`w-8 h-8 rounded-lg bg-gradient-to-br ${step.accent} flex items-center justify-center text-white`}>
                    <step.icon size={16} />
                  </div>
                </div>
                <h3 className="text-base font-semibold text-foreground tracking-tight mb-3">
                  {step.title}
                </h3>
                <div className="mb-1">
                  <span className="text-2xl font-bold font-mono gradient-text">{step.stat}</span>
                </div>
                <p className="text-[10px] font-medium text-muted-foreground/60 tracking-widest mb-3">
                  {step.statLabel}
                </p>
                <p className="text-[13px] text-muted-foreground leading-relaxed">
                  {step.description}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Mobile: single-column pipeline */}
      <div className="sm:hidden">
        <div className="rounded-2xl border border-border bg-background overflow-hidden">
          <div className="relative h-1 bg-border/50">
            <div className="absolute inset-y-0 left-0 right-0 bg-gradient-to-r from-indigo-500 via-violet-500 via-emerald-500 to-amber-500 opacity-60" />
          </div>
          <div className="divide-y divide-border">
            {steps.map((step, i) => (
              <div key={step.title} className="p-5">
                <div className="flex items-center gap-4">
                  <div className={`w-9 h-9 rounded-lg bg-gradient-to-br ${step.accent} flex items-center justify-center text-white flex-shrink-0`}>
                    <step.icon size={18} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3 className="text-sm font-semibold text-foreground tracking-tight">
                      {step.title}
                    </h3>
                    <p className="text-[10px] font-medium text-muted-foreground/60 tracking-widest">
                      STEP {i + 1}
                    </p>
                  </div>
                  <div className="text-right flex-shrink-0">
                    <span className="text-xl font-bold font-mono gradient-text">
                      {step.stat}
                    </span>
                  </div>
                </div>
                <p className="text-[13px] text-muted-foreground leading-relaxed mt-3">
                  {step.description}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </Section>
  );
}
