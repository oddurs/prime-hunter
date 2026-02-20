"use client";

/**
 * @module leaderboard/page
 *
 * Volunteer leaderboard page showing top contributors to the darkreach
 * distributed computing platform. Displays rankings by credit (compute-seconds),
 * primes discovered, team standings, and public fleet statistics.
 *
 * Data fetched from the coordinator's `/api/v1/leaderboard` REST endpoint,
 * refreshed every 30 seconds.
 */

import { useCallback, useEffect, useState } from "react";
import { Award, Crown, Medal, Trophy, Users } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ViewHeader } from "@/components/view-header";
import { API_BASE, numberWithCommas } from "@/lib/format";

/** Single entry on the volunteer leaderboard. */
interface LeaderboardEntry {
  rank: number;
  username: string;
  team: string | null;
  credit: number;
  primes_found: number;
  worker_count: number;
}

/** Aggregate fleet statistics for the public stats banner. */
interface FleetStats {
  totalVolunteers: number;
  totalCredit: number;
  totalPrimes: number;
  totalWorkers: number;
}

export default function LeaderboardPage() {
  const [entries, setEntries] = useState<LeaderboardEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchLeaderboard = useCallback(async () => {
    try {
      const resp = await fetch(`${API_BASE}/api/v1/leaderboard`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data: LeaderboardEntry[] = await resp.json();
      setEntries(data);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load leaderboard");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchLeaderboard();
    const interval = setInterval(fetchLeaderboard, 30_000);
    return () => clearInterval(interval);
  }, [fetchLeaderboard]);

  // Aggregate fleet stats from leaderboard data
  const fleetStats: FleetStats = {
    totalVolunteers: entries.length,
    totalCredit: entries.reduce((sum, e) => sum + e.credit, 0),
    totalPrimes: entries.reduce((sum, e) => sum + e.primes_found, 0),
    totalWorkers: entries.reduce((sum, e) => sum + e.worker_count, 0),
  };

  /** Rank icon for top 3 positions. */
  function rankIcon(rank: number) {
    if (rank === 1) return <Crown className="h-5 w-5 text-yellow-500" />;
    if (rank === 2) return <Medal className="h-5 w-5 text-gray-400" />;
    if (rank === 3) return <Medal className="h-5 w-5 text-amber-600" />;
    return <span className="text-muted-foreground text-sm w-5 text-center">{rank}</span>;
  }

  /** Format credit as human-readable compute-time. */
  function formatCredit(credit: number): string {
    if (credit >= 86400) return `${(credit / 86400).toFixed(1)} core-days`;
    if (credit >= 3600) return `${(credit / 3600).toFixed(1)} core-hours`;
    return `${numberWithCommas(credit)} core-sec`;
  }

  return (
    <div className="p-4 md:p-6 max-w-6xl mx-auto">
      <ViewHeader
        title="Leaderboard"
        subtitle="Top contributors to the darkreach volunteer computing network"
      />

      {/* Fleet stats banner */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-6">
        <Card>
          <CardContent className="p-4 flex items-center gap-3">
            <Users className="h-5 w-5 text-blue-500" />
            <div>
              <p className="text-2xl font-bold">{numberWithCommas(fleetStats.totalVolunteers)}</p>
              <p className="text-xs text-muted-foreground">Volunteers</p>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4 flex items-center gap-3">
            <Trophy className="h-5 w-5 text-yellow-500" />
            <div>
              <p className="text-2xl font-bold">{numberWithCommas(fleetStats.totalPrimes)}</p>
              <p className="text-xs text-muted-foreground">Primes Found</p>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4 flex items-center gap-3">
            <Award className="h-5 w-5 text-green-500" />
            <div>
              <p className="text-2xl font-bold">{formatCredit(fleetStats.totalCredit)}</p>
              <p className="text-xs text-muted-foreground">Total Compute</p>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4 flex items-center gap-3">
            <Users className="h-5 w-5 text-purple-500" />
            <div>
              <p className="text-2xl font-bold">{numberWithCommas(fleetStats.totalWorkers)}</p>
              <p className="text-xs text-muted-foreground">Active Workers</p>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Leaderboard table */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Rankings by Credit</CardTitle>
        </CardHeader>
        <CardContent>
          {loading ? (
            <p className="text-muted-foreground text-center py-8">Loading leaderboard...</p>
          ) : error ? (
            <p className="text-destructive text-center py-8">{error}</p>
          ) : entries.length === 0 ? (
            <div className="text-center py-12">
              <Trophy className="h-12 w-12 text-muted-foreground mx-auto mb-3" />
              <p className="text-muted-foreground">No volunteers yet. Be the first!</p>
              <p className="text-xs text-muted-foreground mt-1">
                Run <code className="bg-muted px-1 rounded">darkreach join</code> to register
              </p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="p-2 w-12">#</th>
                    <th className="p-2">Volunteer</th>
                    <th className="p-2">Team</th>
                    <th className="p-2 text-right">Credit</th>
                    <th className="p-2 text-right">Primes</th>
                    <th className="p-2 text-right">Workers</th>
                  </tr>
                </thead>
                <tbody>
                  {entries.map((entry) => (
                    <tr
                      key={entry.rank}
                      className="border-b last:border-0 hover:bg-muted/50 transition-colors"
                    >
                      <td className="p-2">
                        <div className="flex items-center justify-center">
                          {rankIcon(entry.rank)}
                        </div>
                      </td>
                      <td className="p-2 font-medium">{entry.username}</td>
                      <td className="p-2">
                        {entry.team ? (
                          <Badge variant="outline">{entry.team}</Badge>
                        ) : (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </td>
                      <td className="p-2 text-right font-mono">
                        {formatCredit(entry.credit)}
                      </td>
                      <td className="p-2 text-right">
                        {entry.primes_found > 0 ? (
                          <Badge variant="default">{entry.primes_found}</Badge>
                        ) : (
                          <span className="text-muted-foreground">0</span>
                        )}
                      </td>
                      <td className="p-2 text-right">{entry.worker_count}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
