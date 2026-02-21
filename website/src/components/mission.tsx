import { Infinity, Trophy, Globe, BookOpen } from "lucide-react";
import { Section } from "./ui/section";
import { Card } from "./ui/card";

const reasons = [
  {
    icon: Infinity,
    title: "Permanent Contributions",
    description:
      "Every prime discovered is a mathematical fact that lasts forever. Operator nodes produce results that will be referenced by researchers for centuries.",
  },
  {
    icon: Trophy,
    title: "Break World Records",
    description:
      "Multiple prime form records haven't been challenged in years. Twin primes: 2016. Wagstaff: no organized search at all. These are wide open.",
  },
  {
    icon: Globe,
    title: "Open and Verifiable",
    description:
      "Every discovery comes with a deterministic proof certificate. MIT-licensed, self-hostable, and independently verifiable. No trust required.",
  },
  {
    icon: BookOpen,
    title: "Advance Number Theory",
    description:
      "Prime distribution patterns, conjecture verification, and algorithmic breakthroughs. Each search pushes the frontier of what we know about primes.",
  },
];

export function Mission() {
  return (
    <Section secondary>
      <h2 className="text-3xl font-bold text-foreground mb-4 text-center">
        Why Hunt Primes
      </h2>
      <p className="text-muted-foreground text-center max-w-2xl mx-auto mb-12">
        An operator-owned, AI-orchestrated prime discovery network — contributing
        to one of the oldest open problems in mathematics.
      </p>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
        {reasons.map((reason) => (
          <Card key={reason.title} hover>
            <div className="inline-flex items-center justify-center w-12 h-12 rounded-lg bg-background border border-border text-accent-purple mb-4">
              <reason.icon size={24} />
            </div>
            <h3 className="text-lg font-semibold text-foreground mb-2">
              {reason.title}
            </h3>
            <p className="text-sm text-muted-foreground leading-relaxed">
              {reason.description}
            </p>
          </Card>
        ))}
      </div>
    </Section>
  );
}
