import { primeForms } from "@/lib/prime-forms";
import { Badge } from "./ui/badge";
import { Section } from "./ui/section";
import Link from "next/link";

export function PrimeForms() {
  return (
    <Section id="forms">
      <h2 className="text-3xl font-bold text-foreground mb-4 text-center">
        12 Prime Forms
      </h2>
      <p className="text-muted-foreground text-center max-w-2xl mx-auto mb-12">
        From classical factorial primes to exotic generalized Fermats — darkreach
        searches them all with form-specific sieves, tests, and proofs.
      </p>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {primeForms.map((form) => (
          <div
            key={form.name}
            className="card-glow rounded-lg border border-border bg-card p-5"
          >
            <div className="flex items-start justify-between mb-3">
              <h3 className="text-foreground font-semibold">{form.name}</h3>
              <Badge>{form.algorithm}</Badge>
            </div>
            <div className="font-mono text-accent-purple text-lg mb-3">
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
