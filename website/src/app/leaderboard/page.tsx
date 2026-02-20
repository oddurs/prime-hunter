import { Section } from "@/components/ui/section";
import { Badge } from "@/components/ui/badge";
import {
  contributors,
  teamStandings,
  leaderboardStats,
} from "@/lib/leaderboard-data";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Leaderboard",
  description: "Public contributor rankings for darkreach.",
};

function formatNumber(n: number): string {
  return n.toLocaleString();
}

function RankBadge({ rank }: { rank: number }) {
  if (rank === 1) return <Badge variant="purple">1st</Badge>;
  if (rank === 2) return <Badge variant="green">2nd</Badge>;
  if (rank === 3) return <Badge variant="orange">3rd</Badge>;
  return <span className="text-muted-foreground font-mono text-sm">#{rank}</span>;
}

export default function LeaderboardPage() {
  return (
    <>
      <Section>
        <h1 className="text-4xl font-bold text-foreground mb-4">Leaderboard</h1>
        <p className="text-muted-foreground mb-10">
          Public contributor rankings for the darkreach network.
        </p>

        <div className="grid grid-cols-3 gap-6 mb-12">
          <div className="text-center p-6 rounded-md border border-border bg-card">
            <div className="text-3xl font-bold font-mono text-foreground">
              {formatNumber(leaderboardStats.totalVolunteers)}
            </div>
            <div className="text-sm text-muted-foreground">Total Volunteers</div>
          </div>
          <div className="text-center p-6 rounded-md border border-border bg-card">
            <div className="text-3xl font-bold font-mono text-accent-purple">
              {formatNumber(leaderboardStats.totalPrimes)}
            </div>
            <div className="text-sm text-muted-foreground">Total Primes Found</div>
          </div>
          <div className="text-center p-6 rounded-md border border-border bg-card">
            <div className="text-3xl font-bold font-mono text-foreground">
              {formatNumber(leaderboardStats.totalComputeHours)}h
            </div>
            <div className="text-sm text-muted-foreground">Compute Time</div>
          </div>
        </div>

        <h2 className="text-2xl font-bold text-foreground mb-4">
          Individual Rankings
        </h2>
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-card text-muted-foreground text-left">
                <th className="px-4 py-3 font-medium w-16">Rank</th>
                <th className="px-4 py-3 font-medium">Username</th>
                <th className="px-4 py-3 font-medium">Team</th>
                <th className="px-4 py-3 font-medium text-right">Credit</th>
                <th className="px-4 py-3 font-medium text-right">Primes</th>
                <th className="px-4 py-3 font-medium text-right">Hours</th>
              </tr>
            </thead>
            <tbody>
              {contributors.map((c) => (
                <tr
                  key={c.username}
                  className="border-t border-border hover:bg-card/50 transition-colors"
                >
                  <td className="px-4 py-3">
                    <RankBadge rank={c.rank} />
                  </td>
                  <td className="px-4 py-3 font-mono text-foreground">
                    {c.username}
                  </td>
                  <td className="px-4 py-3 text-muted-foreground">{c.team}</td>
                  <td className="px-4 py-3 font-mono text-foreground text-right">
                    {formatNumber(c.credit)}
                  </td>
                  <td className="px-4 py-3 font-mono text-accent-purple text-right">
                    {formatNumber(c.primesFound)}
                  </td>
                  <td className="px-4 py-3 font-mono text-muted-foreground text-right">
                    {formatNumber(c.computeHours)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Section>

      <Section secondary>
        <h2 className="text-2xl font-bold text-foreground mb-4">Team Standings</h2>
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-background text-muted-foreground text-left">
                <th className="px-4 py-3 font-medium w-16">Rank</th>
                <th className="px-4 py-3 font-medium">Team</th>
                <th className="px-4 py-3 font-medium text-right">Members</th>
                <th className="px-4 py-3 font-medium text-right">
                  Total Credit
                </th>
                <th className="px-4 py-3 font-medium text-right">
                  Total Primes
                </th>
              </tr>
            </thead>
            <tbody>
              {teamStandings.map((team) => (
                <tr key={team.name} className="border-t border-border">
                  <td className="px-4 py-3">
                    <RankBadge rank={team.rank} />
                  </td>
                  <td className="px-4 py-3 font-semibold text-foreground">
                    {team.name}
                  </td>
                  <td className="px-4 py-3 text-muted-foreground text-right">
                    {team.members}
                  </td>
                  <td className="px-4 py-3 font-mono text-foreground text-right">
                    {formatNumber(team.totalCredit)}
                  </td>
                  <td className="px-4 py-3 font-mono text-accent-purple text-right">
                    {formatNumber(team.totalPrimes)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Section>
    </>
  );
}
