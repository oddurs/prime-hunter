import { DarkReachLogo } from "./darkreach-logo";

export function Hero() {
  return (
    <section className="relative min-h-screen flex items-center justify-center dot-grid overflow-hidden">
      {/* Purple radial glow behind the Σ */}
      <div
        className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full pointer-events-none"
        style={{
          background:
            "radial-gradient(circle, rgba(188,140,255,0.15) 0%, rgba(188,140,255,0.05) 40%, transparent 70%)",
        }}
      />

      <div className="relative z-10 text-center px-6 max-w-3xl">
        {/* Large Σ */}
        <div className="flex justify-center mb-8">
          <DarkReachLogo size={160} glow />
        </div>

        <h1 className="text-5xl sm:text-6xl font-bold tracking-tight text-text mb-6">
          Distributed Prime Discovery
        </h1>

        <p className="text-lg sm:text-xl text-text-muted max-w-2xl mx-auto mb-10">
          Hunt special-form primes across CPU clusters. 12 algorithms.
          Deterministic proofs. Open source.
        </p>

        {/* CTAs */}
        <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
          <a
            href="#get-started"
            className="inline-flex items-center px-6 py-3 rounded-md bg-accent-purple text-white font-medium hover:opacity-90 transition-opacity"
          >
            Get Started
          </a>
          <a
            href="https://github.com/darkreach/darkreach"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center px-6 py-3 rounded-md border border-border text-text-muted font-medium hover:text-text hover:border-text-muted transition-colors"
          >
            <svg
              width="20"
              height="20"
              viewBox="0 0 16 16"
              fill="currentColor"
              className="mr-2"
            >
              <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
            </svg>
            View on GitHub
          </a>
        </div>
      </div>
    </section>
  );
}
