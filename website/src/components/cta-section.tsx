import { Section } from "./ui/section";
import Link from "next/link";
import { ArrowRight, Github, Download, Settings, Rocket } from "lucide-react";

const steps = [
  {
    icon: Download,
    title: "Install",
    code: "cargo install darkreach",
  },
  {
    icon: Settings,
    title: "Connect",
    code: "export DATABASE_URL=postgres://...",
  },
  {
    icon: Rocket,
    title: "Hunt",
    code: "darkreach kbn --k 3 --base 2 --min-n 60000",
  },
];

export function CtaSection() {
  return (
    <Section className="cta-gradient">
      <div className="text-center max-w-3xl mx-auto">
        <p className="text-sm font-medium text-accent-purple uppercase tracking-wider mb-3">
          Get started
        </p>
        <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
          Up and running in three commands
        </h2>
        <p className="text-muted-foreground max-w-xl mx-auto mb-12">
          Single binary, no runtime dependencies. MIT licensed.
        </p>

        {/* Three-step walkthrough */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-14">
          {steps.map((step, i) => (
            <div
              key={step.title}
              className="relative rounded-xl border border-border bg-card/60 backdrop-blur-sm p-5 text-left"
            >
              <div className="flex items-center gap-3 mb-3">
                <span className="flex items-center justify-center w-7 h-7 rounded-full bg-accent-purple/10 text-accent-purple text-xs font-bold border border-accent-purple/20">
                  {i + 1}
                </span>
                <span className="text-sm font-semibold text-foreground">
                  {step.title}
                </span>
              </div>
              <code className="block rounded-lg bg-background/80 border border-border px-3 py-2.5 font-mono text-[13px] text-accent-green truncate">
                {step.code}
              </code>
            </div>
          ))}
        </div>

        {/* CTA buttons */}
        <div className="flex flex-col sm:flex-row items-center justify-center gap-3">
          <Link
            href="/download"
            className="group inline-flex items-center gap-2 px-8 py-3.5 rounded-lg bg-accent-purple text-white font-medium hover:bg-accent-purple/90 transition-colors shadow-lg shadow-accent-purple/20 text-lg"
          >
            Start Hunting
            <ArrowRight size={18} className="group-hover:translate-x-0.5 transition-transform" />
          </Link>
          <a
            href="https://github.com/darkreach/darkreach"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-8 py-3.5 rounded-lg border border-border text-muted-foreground font-medium hover:text-foreground hover:border-muted-foreground/60 transition-colors text-lg"
          >
            <Github size={18} />
            View on GitHub
          </a>
        </div>
      </div>
    </Section>
  );
}
