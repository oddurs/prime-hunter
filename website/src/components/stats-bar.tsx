const stats = [
  { label: "Primes Found", value: "2,847", live: true },
  { label: "Candidates Tested", value: "14.2B", live: true },
  { label: "Active Workers", value: "38", live: true },
  { label: "Compute Hours", value: "127K", live: true },
  { label: "Search Forms", value: "12", live: false },
];

export function StatsBar() {
  return (
    <section className="border-y border-border bg-bg-secondary">
      <div className="mx-auto max-w-6xl px-6 py-8">
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-6">
          {stats.map((stat) => (
            <div key={stat.label} className="text-center">
              <div className="flex items-center justify-center gap-2 mb-1">
                {stat.live && (
                  <span className="inline-block w-2 h-2 rounded-full bg-accent-green pulse-green" />
                )}
                <span className="text-3xl font-bold font-mono text-text">
                  {stat.value}
                </span>
              </div>
              <div className="text-sm text-text-muted">{stat.label}</div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
