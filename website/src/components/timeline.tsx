interface TimelineEvent {
  date: string;
  title: string;
  description: string;
}

const events: TimelineEvent[] = [
  {
    date: "2025 Q4",
    title: "Project inception",
    description:
      "Started as a factorial prime hunter in Rust with GMP. First primes found within days.",
  },
  {
    date: "2025 Q4",
    title: "Core engine complete",
    description:
      "Implemented 4 prime forms (factorial, palindromic, k*b^n, near-repdigit) with Pocklington/Morrison proofs.",
  },
  {
    date: "2026 Jan",
    title: "Expanded to 12 forms",
    description:
      "Added primorial, Cullen/Woodall, Wagstaff, Carol/Kynea, twin, Sophie Germain, repunit, and generalized Fermat searches.",
  },
  {
    date: "2026 Jan",
    title: "Fleet infrastructure",
    description:
      "Built coordinator/worker architecture with PostgreSQL work distribution and real-time dashboard.",
  },
  {
    date: "2026 Feb",
    title: "Rebranded to darkreach",
    description:
      "Repositioned as an AI-driven distributed computing platform. Launched darkreach.ai with docs and status page.",
  },
  {
    date: "2026 Q1",
    title: "AI agent integration",
    description:
      "Autonomous agents for research, strategy optimization, and campaign orchestration.",
  },
];

export function Timeline() {
  return (
    <div className="space-y-0">
      {events.map((event, i) => (
        <div key={i} className="flex gap-4">
          <div className="flex flex-col items-center">
            <div className="w-3 h-3 rounded-full bg-accent-purple shrink-0 mt-1.5" />
            {i < events.length - 1 && (
              <div className="w-px flex-1 bg-border" />
            )}
          </div>
          <div className="pb-8">
            <span className="text-xs font-mono text-text-muted">
              {event.date}
            </span>
            <h3 className="text-text font-semibold mt-1">{event.title}</h3>
            <p className="text-sm text-text-muted mt-1">{event.description}</p>
          </div>
        </div>
      ))}
    </div>
  );
}
