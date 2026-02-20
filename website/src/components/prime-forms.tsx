import { primeForms } from "@/lib/prime-forms";

export function PrimeForms() {
  return (
    <section id="forms" className="py-24 px-6">
      <div className="mx-auto max-w-6xl">
        <h2 className="text-3xl font-bold text-text mb-4 text-center">
          12 Prime Forms
        </h2>
        <p className="text-text-muted text-center max-w-2xl mx-auto mb-12">
          From classical factorial primes to exotic generalized Fermats — Darkreach
          searches them all with form-specific sieves, tests, and proofs.
        </p>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {primeForms.map((form) => (
            <div
              key={form.name}
              className="card-glow rounded-lg border border-border bg-bg-secondary p-5"
            >
              <div className="flex items-start justify-between mb-3">
                <h3 className="text-text font-semibold">{form.name}</h3>
                <span className="text-xs font-mono px-2 py-0.5 rounded-full bg-bg border border-border text-text-muted">
                  {form.algorithm}
                </span>
              </div>
              <div className="font-mono text-accent-purple text-lg mb-3">
                {form.formula}
              </div>
              <p className="text-sm text-text-muted leading-relaxed">
                {form.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
