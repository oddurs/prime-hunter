/**
 * @module record-comparison
 *
 * Compact card comparing our best prime against the world record for a
 * given form. Shows a progress bar indicating how close we are to the
 * record in digit count.
 */

import { Card, CardContent } from "@/components/ui/card";
import { numberWithCommas, formLabels } from "@/lib/format";

interface RecordSummary {
  form: string;
  expression: string;
  digits: number;
  holder: string | null;
  our_best_digits: number;
}

export function RecordComparison({ record }: { record: RecordSummary }) {
  const pct =
    record.digits > 0
      ? Math.min(100, (record.our_best_digits / record.digits) * 100)
      : 0;
  const label = formLabels[record.form] ?? record.form;
  const expr =
    record.expression.length > 28
      ? record.expression.slice(0, 25) + "..."
      : record.expression;

  return (
    <Card>
      <CardContent className="py-3 px-4">
        <div className="flex items-center justify-between mb-1">
          <span className="text-xs font-medium">{label}</span>
          <span className="text-xs text-muted-foreground">
            {numberWithCommas(record.digits)} digits
          </span>
        </div>
        <div className="text-[11px] text-muted-foreground truncate mb-2">
          {expr} — {record.holder ?? "unknown"}
        </div>
        <div className="h-1.5 rounded-full bg-muted overflow-hidden">
          <div
            className="h-full rounded-full bg-[#f78166] transition-all"
            style={{ width: `${pct}%` }}
          />
        </div>
        <div className="flex justify-between mt-1 text-[10px] text-muted-foreground">
          <span>
            Our best:{" "}
            {record.our_best_digits > 0
              ? numberWithCommas(record.our_best_digits)
              : "none"}
          </span>
          <span>{pct > 0 ? `${pct.toFixed(1)}%` : "-"}</span>
        </div>
      </CardContent>
    </Card>
  );
}
