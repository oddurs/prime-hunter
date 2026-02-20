import { DarkReachLogo } from "./darkreach-logo";
import Link from "next/link";

export function Hero() {
  return (
    <section className="relative min-h-[90vh] flex items-center justify-center dot-grid overflow-hidden">
      {/* Purple radial glow */}
      <div
        className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full pointer-events-none"
        style={{
          background:
            "radial-gradient(circle, rgba(188,140,255,0.15) 0%, rgba(188,140,255,0.05) 40%, transparent 70%)",
        }}
      />

      <div className="relative z-10 text-center px-6 max-w-3xl">
        <div className="flex justify-center mb-8">
          <DarkReachLogo size={120} glow />
        </div>

        <h1 className="text-5xl sm:text-6xl font-bold tracking-tight text-text mb-6">
          AI-driven distributed computing.
        </h1>

        <p className="text-lg sm:text-xl text-text-muted max-w-2xl mx-auto mb-10">
          darkreach autonomously researches, optimizes, and orchestrates
          scientific discoveries across a fleet of servers. Currently hunting
          primes.
        </p>

        <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
          <Link
            href="/download"
            className="inline-flex items-center px-6 py-3 rounded-md bg-accent-purple text-white font-medium hover:opacity-90 transition-opacity"
          >
            Get Started
          </Link>
          <a
            href="https://app.darkreach.ai"
            className="inline-flex items-center px-6 py-3 rounded-md border border-border text-text-muted font-medium hover:text-text hover:border-text-muted transition-colors"
          >
            Open Dashboard
          </a>
        </div>
      </div>
    </section>
  );
}
