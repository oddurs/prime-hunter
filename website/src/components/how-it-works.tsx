const steps = [
  {
    icon: (
      <svg
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M4 4h16v16H4z" />
        <path d="M4 8h16" />
        <path d="M8 4v16" />
      </svg>
    ),
    title: "Sieve",
    description:
      "Eliminate composites with form-specific sieves — wheel factorization, BSGS, and Pollard P−1 filtering before any heavy computation.",
  },
  {
    icon: (
      <svg
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <circle cx="12" cy="12" r="10" />
        <path d="M12 6v6l4 2" />
      </svg>
    ),
    title: "Test",
    description:
      "Run Miller-Rabin pre-screening, then form-specific primality tests — Proth, LLR, Pépin — accelerated by PFGW and GWNUM FFT.",
  },
  {
    icon: (
      <svg
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M9 12l2 2 4-4" />
        <path d="M12 3l9 4.5v5c0 4.97-3.67 9.14-9 10.5-5.33-1.36-9-5.53-9-10.5v-5L12 3z" />
      </svg>
    ),
    title: "Prove",
    description:
      "Generate deterministic primality certificates — Pocklington, Morrison, BLS — with independently verifiable witness data.",
  },
];

export function HowItWorks() {
  return (
    <section id="features" className="py-24 px-6 bg-bg-secondary">
      <div className="mx-auto max-w-6xl">
        <h2 className="text-3xl font-bold text-text mb-4 text-center">
          How It Works
        </h2>
        <p className="text-text-muted text-center max-w-2xl mx-auto mb-12">
          Every candidate passes through a three-stage pipeline optimized for
          each prime form.
        </p>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
          {steps.map((step, i) => (
            <div key={step.title} className="text-center">
              <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-bg border border-border text-accent-purple mb-4">
                {step.icon}
              </div>
              <div className="flex items-center justify-center gap-2 mb-2">
                <span className="text-xs font-mono text-text-muted">
                  0{i + 1}
                </span>
                <h3 className="text-xl font-semibold text-text">
                  {step.title}
                </h3>
              </div>
              <p className="text-text-muted text-sm leading-relaxed max-w-xs mx-auto">
                {step.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
