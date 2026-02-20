import { Section } from "./ui/section";

const features = [
  {
    feature: "Prime forms supported",
    darkreach: "12",
    gimps: "1 (Mersenne)",
    primegrid: "~6",
  },
  {
    feature: "Deterministic proofs",
    darkreach: "Yes (Pocklington, Morrison, BLS)",
    gimps: "Yes (Lucas-Lehmer)",
    primegrid: "Partial",
  },
  {
    feature: "AI-driven orchestration",
    darkreach: "Yes (autonomous agents)",
    gimps: "No",
    primegrid: "No",
  },
  {
    feature: "Modern dashboard",
    darkreach: "Yes (real-time, charts, search mgmt)",
    gimps: "Basic web",
    primegrid: "BOINC client",
  },
  {
    feature: "Open source",
    darkreach: "Yes (MIT)",
    gimps: "No",
    primegrid: "Partially",
  },
  {
    feature: "Self-hostable",
    darkreach: "Yes (single binary + Postgres)",
    gimps: "No",
    primegrid: "No",
  },
  {
    feature: "Proof certificates",
    darkreach: "JSONB witnesses, independently verifiable",
    gimps: "Internal",
    primegrid: "None",
  },
];

export function Comparison() {
  return (
    <Section secondary>
      <h2 className="text-3xl font-bold text-text mb-4 text-center">
        Why darkreach
      </h2>
      <p className="text-text-muted text-center max-w-2xl mx-auto mb-12">
        How darkreach compares to existing distributed prime search platforms.
      </p>

      <div className="overflow-x-auto rounded-lg border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-bg text-text-muted text-left">
              <th className="px-4 py-3 font-medium">Feature</th>
              <th className="px-4 py-3 font-medium">
                <span className="text-accent-purple">darkreach</span>
              </th>
              <th className="px-4 py-3 font-medium">GIMPS</th>
              <th className="px-4 py-3 font-medium">PrimeGrid</th>
            </tr>
          </thead>
          <tbody>
            {features.map((row) => (
              <tr key={row.feature} className="border-t border-border">
                <td className="px-4 py-3 text-text font-medium">
                  {row.feature}
                </td>
                <td className="px-4 py-3 text-accent-green">
                  {row.darkreach}
                </td>
                <td className="px-4 py-3 text-text-muted">{row.gimps}</td>
                <td className="px-4 py-3 text-text-muted">{row.primegrid}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Section>
  );
}
