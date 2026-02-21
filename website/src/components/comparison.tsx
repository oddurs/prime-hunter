import { Check, X, Minus } from "lucide-react";
import { Section } from "./ui/section";

type Status = "yes" | "no" | "partial";

interface Row {
  feature: string;
  darkreach: Status;
  darkreachDetail?: string;
  gimps: Status;
  gimpsDetail?: string;
  primegrid: Status;
  primegridDetail?: string;
}

const rows: Row[] = [
  {
    feature: "Multiple prime forms",
    darkreach: "yes", darkreachDetail: "12 forms",
    gimps: "no", gimpsDetail: "Mersenne only",
    primegrid: "partial", primegridDetail: "~6 forms",
  },
  {
    feature: "AI orchestration",
    darkreach: "yes", darkreachDetail: "Autonomous agents",
    gimps: "no",
    primegrid: "no",
  },
  {
    feature: "Open source",
    darkreach: "yes", darkreachDetail: "MIT license",
    gimps: "no",
    primegrid: "partial",
  },
  {
    feature: "Self-hostable",
    darkreach: "yes", darkreachDetail: "Single binary",
    gimps: "no",
    primegrid: "no",
  },
  {
    feature: "Real-time dashboard",
    darkreach: "yes", darkreachDetail: "WebSocket + charts",
    gimps: "partial", gimpsDetail: "Basic web UI",
    primegrid: "no", primegridDetail: "BOINC client",
  },
  {
    feature: "Proof certificates",
    darkreach: "yes", darkreachDetail: "Verifiable JSONB",
    gimps: "partial", gimpsDetail: "Internal only",
    primegrid: "no",
  },
];

function StatusIcon({ status }: { status: Status }) {
  if (status === "yes") return <Check size={16} className="text-accent-green" />;
  if (status === "partial") return <Minus size={16} className="text-amber-400" />;
  return <X size={16} className="text-muted-foreground/40" />;
}

export function Comparison() {
  return (
    <Section secondary>
      <div className="text-center mb-16">
        <p className="text-sm font-medium text-accent-purple uppercase tracking-wider mb-3">
          Comparison
        </p>
        <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
          Why darkreach
        </h2>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          The first modern, AI-driven prime search platform.
        </p>
      </div>

      <div className="max-w-3xl mx-auto overflow-x-auto rounded-xl border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-background/50">
              <th className="px-5 py-3.5 text-left font-medium text-muted-foreground">Feature</th>
              <th className="px-5 py-3.5 text-center font-semibold text-accent-purple">darkreach</th>
              <th className="px-5 py-3.5 text-center font-medium text-muted-foreground">GIMPS</th>
              <th className="px-5 py-3.5 text-center font-medium text-muted-foreground">PrimeGrid</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.feature} className="border-t border-border">
                <td className="px-5 py-3.5 text-foreground font-medium">{row.feature}</td>
                <td className="px-5 py-3.5">
                  <div className="flex flex-col items-center gap-0.5">
                    <StatusIcon status={row.darkreach} />
                    {row.darkreachDetail && (
                      <span className="text-[11px] text-accent-green">{row.darkreachDetail}</span>
                    )}
                  </div>
                </td>
                <td className="px-5 py-3.5">
                  <div className="flex flex-col items-center gap-0.5">
                    <StatusIcon status={row.gimps} />
                    {row.gimpsDetail && (
                      <span className="text-[11px] text-muted-foreground">{row.gimpsDetail}</span>
                    )}
                  </div>
                </td>
                <td className="px-5 py-3.5">
                  <div className="flex flex-col items-center gap-0.5">
                    <StatusIcon status={row.primegrid} />
                    {row.primegridDetail && (
                      <span className="text-[11px] text-muted-foreground">{row.primegridDetail}</span>
                    )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Section>
  );
}
