"use client";

/**
 * @module form-leaderboard
 *
 * Per-form analytics table for the dashboard Insights section. Replaces the
 * simple "by form" badge list with a sortable table showing:
 *
 * | Form | Count | Largest | Last Found | Verified |
 *
 * Data comes from the `get_form_leaderboard()` Supabase RPC via the
 * `useFormLeaderboard` hook. Columns are clickable to re-sort.
 *
 * "Last Found" displays as relative time (e.g., "2h 15m ago").
 * "Verified" shows a percentage with color coding (green ≥90%, yellow ≥50%).
 */

import { useState } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { numberWithCommas, relativeTime, formLabels } from "@/lib/format";
import type { FormLeaderboardEntry } from "@/hooks/use-form-leaderboard";

type SortKey = "count" | "largest_digits" | "latest_found_at" | "verified_pct";
type SortDir = "asc" | "desc";

interface FormLeaderboardProps {
  entries: FormLeaderboardEntry[];
}

export function FormLeaderboard({ entries }: FormLeaderboardProps) {
  const [sortKey, setSortKey] = useState<SortKey>("count");
  const [sortDir, setSortDir] = useState<SortDir>("desc");

  function handleSort(key: SortKey) {
    if (sortKey === key) {
      setSortDir(sortDir === "asc" ? "desc" : "asc");
    } else {
      setSortKey(key);
      setSortDir("desc");
    }
  }

  function sortIndicator(key: SortKey) {
    if (sortKey !== key) return " \u2195";
    return sortDir === "asc" ? " \u2191" : " \u2193";
  }

  const sorted = [...entries].sort((a, b) => {
    const mul = sortDir === "asc" ? 1 : -1;
    if (sortKey === "latest_found_at") {
      return mul * (new Date(a[sortKey]).getTime() - new Date(b[sortKey]).getTime());
    }
    return mul * ((a[sortKey] as number) - (b[sortKey] as number));
  });

  if (entries.length === 0) {
    return null;
  }

  return (
    <Card className="mb-4 py-0 overflow-hidden">
      <CardContent className="p-0">
        <div className="px-4 py-2.5 border-b">
          <h3 className="text-sm font-semibold text-foreground">
            Form Leaderboard
          </h3>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b text-xs text-muted-foreground">
                <th className="text-left font-medium px-4 py-2">Form</th>
                <th
                  className="text-right font-medium px-4 py-2 cursor-pointer select-none hover:text-foreground"
                  onClick={() => handleSort("count")}
                >
                  Count{sortIndicator("count")}
                </th>
                <th
                  className="text-right font-medium px-4 py-2 cursor-pointer select-none hover:text-foreground"
                  onClick={() => handleSort("largest_digits")}
                >
                  Largest{sortIndicator("largest_digits")}
                </th>
                <th
                  className="text-right font-medium px-4 py-2 cursor-pointer select-none hover:text-foreground"
                  onClick={() => handleSort("latest_found_at")}
                >
                  Last Found{sortIndicator("latest_found_at")}
                </th>
                <th
                  className="text-right font-medium px-4 py-2 cursor-pointer select-none hover:text-foreground"
                  onClick={() => handleSort("verified_pct")}
                >
                  Verified{sortIndicator("verified_pct")}
                </th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((entry) => (
                <tr
                  key={entry.form}
                  className="border-b last:border-0 hover:bg-muted/50"
                >
                  <td className="px-4 py-2">
                    <Badge variant="outline" className="font-normal">
                      {formLabels[entry.form] ?? entry.form}
                    </Badge>
                  </td>
                  <td className="text-right px-4 py-2 tabular-nums font-medium">
                    {numberWithCommas(entry.count)}
                  </td>
                  <td className="text-right px-4 py-2 tabular-nums">
                    {numberWithCommas(entry.largest_digits)} digits
                  </td>
                  <td className="text-right px-4 py-2 text-muted-foreground">
                    {relativeTime(entry.latest_found_at)}
                  </td>
                  <td className="text-right px-4 py-2">
                    <span
                      className={`tabular-nums font-medium ${
                        entry.verified_pct >= 90
                          ? "text-green-500"
                          : entry.verified_pct >= 50
                            ? "text-yellow-500"
                            : "text-red-500"
                      }`}
                    >
                      {entry.verified_pct}%
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  );
}
