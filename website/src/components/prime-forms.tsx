import { primeForms } from "@/lib/prime-forms";
import { Section } from "./ui/section";
import Link from "next/link";

const accentColors = [
  "from-indigo-500 to-violet-500",
  "from-emerald-500 to-teal-500",
  "from-amber-500 to-orange-500",
  "from-violet-500 to-purple-500",
  "from-cyan-500 to-blue-500",
  "from-rose-500 to-pink-500",
  "from-sky-500 to-indigo-500",
  "from-lime-500 to-emerald-500",
  "from-orange-500 to-red-500",
  "from-teal-500 to-cyan-500",
  "from-pink-500 to-rose-500",
  "from-purple-500 to-indigo-500",
];

export function PrimeForms() {
  return (
    <Section id="forms">
      <div className="text-center mb-16">
        <p className="text-sm font-medium text-accent-purple uppercase tracking-wider mb-3">
          Prime Forms
        </p>
        <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
          12 specialized searches
        </h2>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          Each form has a dedicated sieve, primality test, and proof strategy.
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {primeForms.map((form, i) => (
          <div
            key={form.name}
            className="group relative rounded-xl border border-border bg-card p-5 transition-all hover:border-border/80 hover:shadow-lg hover:shadow-accent-purple/5"
          >
            {/* Animated gradient top line */}
            <div className="absolute inset-x-0 top-0 h-px">
              <div className={`h-full w-0 group-hover:w-full bg-gradient-to-r ${accentColors[i % accentColors.length]} transition-all duration-500 rounded-t-xl`} />
            </div>

            <div className="flex items-start justify-between mb-2">
              <h3 className="text-foreground font-semibold">{form.name}</h3>
              <span className="text-[11px] font-medium text-muted-foreground bg-background border border-border rounded-md px-2 py-0.5">
                {form.algorithm}
              </span>
            </div>
            <div className="font-mono text-lg gradient-text mb-3">
              {form.formula}
            </div>
            <p className="text-sm text-muted-foreground leading-relaxed mb-3">
              {form.description}
            </p>
            <Link
              href="/docs/prime-forms"
              className="text-xs text-primary hover:underline"
            >
              Learn more →
            </Link>
          </div>
        ))}
      </div>
    </Section>
  );
}
