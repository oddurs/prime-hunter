"use client";

import { useEffect, useState } from "react";
import dynamic from "next/dynamic";
import { HeroLogo } from "./hero-logo";
import Link from "next/link";
import { ArrowRight } from "lucide-react";

const NodeNetwork = dynamic(
  () => import("./node-network").then((m) => m.NodeNetwork),
  { ssr: false }
);

const headlines = [
  "Hunting record-breaking primes.",
  "12 prime forms. One engine.",
  "AI agents that optimize themselves.",
  "Deterministic proofs, not guesses.",
  "Distributed across a global fleet.",
];

export function Hero() {
  const [index, setIndex] = useState(0);
  const [fade, setFade] = useState(true);

  useEffect(() => {
    const timer = setInterval(() => {
      setFade(false);
      setTimeout(() => {
        setIndex((prev) => (prev + 1) % headlines.length);
        setFade(true);
      }, 300);
    }, 4000);
    return () => clearInterval(timer);
  }, []);

  return (
    <section className="relative min-h-[calc(100vh-4rem)] flex flex-col items-center justify-center bg-background overflow-hidden">
      {/* Three.js node network background */}
      <NodeNetwork />

      {/* Ambient glow orbs */}
      <div
        className="absolute top-1/3 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[600px] rounded-full pointer-events-none z-[1]"
        style={{
          background:
            "radial-gradient(circle, rgba(99,102,241,0.18) 0%, rgba(99,102,241,0.04) 50%, transparent 70%)",
        }}
      />
      <div
        className="absolute bottom-1/4 right-1/4 w-[400px] h-[400px] rounded-full pointer-events-none z-[1]"
        style={{
          background:
            "radial-gradient(circle, rgba(52,211,153,0.06) 0%, transparent 60%)",
        }}
      />

      <div className="relative z-10 text-center px-6 max-w-4xl">
        <div className="flex justify-center mb-10">
          <HeroLogo size={96} />
        </div>

        <h1 className="text-5xl sm:text-6xl lg:text-7xl font-bold tracking-[-0.03em] text-foreground mb-4 leading-[1.08]">
          AI-driven prime discovery
          <br />
          <span className="gradient-text">at unprecedented scale.</span>
        </h1>

        <div className="h-8 mb-6">
          <p
            className={`text-lg sm:text-xl text-muted-foreground transition-opacity duration-300 ${
              fade ? "opacity-100" : "opacity-0"
            }`}
          >
            {headlines[index]}
          </p>
        </div>

        <p className="text-lg sm:text-xl text-muted-foreground/80 max-w-2xl mx-auto mb-10">
          Autonomous agents research strategies, orchestrate fleets, and
          generate mathematical proofs. Open source, self-hostable, MIT licensed.
        </p>

        <div className="flex flex-col sm:flex-row items-center justify-center gap-3 mb-16">
          <Link
            href="/download"
            className="group inline-flex items-center gap-2 px-7 py-3 rounded-lg bg-accent-purple text-white font-medium hover:bg-accent-purple/90 transition-colors shadow-lg shadow-accent-purple/20"
          >
            Get Started
            <ArrowRight size={16} className="group-hover:translate-x-0.5 transition-transform" />
          </Link>
          <a
            href="https://app.darkreach.ai"
            className="inline-flex items-center px-7 py-3 rounded-lg border border-border text-muted-foreground font-medium hover:text-foreground hover:border-muted-foreground/60 transition-colors"
          >
            Open Dashboard
          </a>
        </div>
      </div>
    </section>
  );
}
