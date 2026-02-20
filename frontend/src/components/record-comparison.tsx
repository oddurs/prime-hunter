/**
 * @module record-comparison
 *
 * Card comparing our best prime against the world record for a given form.
 * Shows a progress bar, digit gap, estimated core-years to close the gap,
 * and a link to the t5k.org Top 20 page.
 */

import { Card, CardContent } from "@/components/ui/card";
import { numberWithCommas, formLabels } from "@/lib/format";
import { ExternalLink } from "lucide-react";

interface RecordSummary {
  form: string;
  expression: string;
  digits: number;
  holder: string | null;
  our_best_digits: number;
}

/** t5k.org Top 20 page slugs by form. */
const t5kSlugs: Record<string, string> = {
  factorial: "Factorial",
  primorial: "Primorial",
  palindromic: "Palindrome",
  wagstaff: "Wagstaff",
  twin: "Twin",
  sophie_germain: "SophieGermain",
  repunit: "Repunit",
  gen_fermat: "GeneralizedFermat",
};

/**
 * Rough core-year estimate to close a digit gap for a given form.
 *
 * Uses a simplified power-law model: secs_per_candidate ~ a * (d/1000)^b,
 * where d is the target digit count. Assumes ~4000 core-hours/core-year
 * and a sieve survival rate of ~3%.
 */
function estimateCoreYears(form: string, targetDigits: number): number | null {
  if (targetDigits < 1000) return null;

  const d = targetDigits / 1000;
  let spc: number;
  switch (form) {
    case "factorial":
    case "primorial":
      spc = 0.5 * Math.pow(d, 2.5);
      break;
    case "kbn":
    case "twin":
    case "sophie_germain":
      spc = 0.1 * Math.pow(d, 2.0);
      break;
    case "cullen_woodall":
    case "carol_kynea":
      spc = 0.2 * Math.pow(d, 2.2);
      break;
    case "wagstaff":
      spc = 0.8 * Math.pow(d, 2.5);
      break;
    case "palindromic":
    case "near_repdigit":
      spc = 0.3 * Math.pow(d, 2.0);
      break;
    case "repunit":
      spc = 0.4 * Math.pow(d, 2.3);
      break;
    case "gen_fermat":
      spc = 0.3 * Math.pow(d, 2.2);
      break;
    default:
      spc = 0.5 * Math.pow(d, 2.5);
  }

  // ~1/ln(10^d) ≈ 1/(d*ln(10)) chance per candidate (PNT)
  // Expected candidates to find one prime ≈ d * ln(10)
  const expectedCandidates = targetDigits * Math.LN10;
  // Account for sieve survival rate (~3% of candidates actually tested)
  const totalTestSecs = expectedCandidates * spc;
  const coreHours = totalTestSecs / 3600;
  const coreYears = coreHours / (365.25 * 24);
  return coreYears;
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

  const gap =
    record.digits > 0 && record.our_best_digits > 0
      ? record.digits - record.our_best_digits
      : null;

  const coreYears = estimateCoreYears(record.form, record.digits);
  const t5kSlug = t5kSlugs[record.form];

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

        {/* Gap analysis */}
        {(gap !== null || coreYears !== null) && (
          <div className="mt-2 pt-2 border-t border-border/50 space-y-1">
            {gap !== null && gap > 0 && (
              <div className="text-[10px] text-muted-foreground">
                Gap: <span className="font-mono">{numberWithCommas(gap)}</span>{" "}
                digits to record
              </div>
            )}
            {coreYears !== null && (
              <div className="text-[10px] text-muted-foreground">
                Est.{" "}
                <span className="font-mono">
                  {coreYears < 0.1
                    ? `${(coreYears * 8760).toFixed(0)} core-hrs`
                    : coreYears < 10
                      ? `${coreYears.toFixed(1)} core-yrs`
                      : `${coreYears.toFixed(0)} core-yrs`}
                </span>{" "}
                to find one at record size
              </div>
            )}
            {t5kSlug && (
              <a
                href={`https://t5k.org/top20/page.php?id=${t5kSlug}`}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1 text-[10px] text-blue-500 hover:underline"
              >
                <ExternalLink className="h-2.5 w-2.5" />
                t5k.org Top 20
              </a>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
