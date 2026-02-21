"use client";

import { useEffect, useState } from "react";
import { Section } from "./ui/section";
import { ExternalLink } from "lucide-react";

const API_BASE = "https://api.darkreach.ai";

interface PrimeEntry {
  id: number;
  form: string;
  expression: string;
  digits: number;
  discovered_at: string;
}

const FALLBACK_ENTRIES: PrimeEntry[] = [
  { id: 1, form: "kbn", expression: "3 \u00b7 2^59973 + 1", digits: 18055, discovered_at: "2026-02-18T14:22:00Z" },
  { id: 2, form: "palindromic", expression: "10^502 + R(501)^rev + 1", digits: 503, discovered_at: "2026-02-18T13:05:00Z" },
  { id: 3, form: "kbn", expression: "3 \u00b7 2^59941 - 1", digits: 18046, discovered_at: "2026-02-18T11:30:00Z" },
  { id: 4, form: "palindromic", expression: "10^498 + R(497)^rev + 1", digits: 499, discovered_at: "2026-02-17T22:15:00Z" },
  { id: 5, form: "kbn", expression: "3 \u00b7 2^59887 + 1", digits: 18029, discovered_at: "2026-02-17T19:40:00Z" },
  { id: 6, form: "kbn", expression: "3 \u00b7 2^59851 - 1", digits: 18018, discovered_at: "2026-02-17T16:10:00Z" },
];

function timeAgo(iso: string): string {
  const secs = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
  if (secs < 60) return "just now";
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86400)}d ago`;
}

function formLabel(form: string): string {
  const labels: Record<string, string> = {
    kbn: "k\u00b7b^n\u00b11",
    palindromic: "Palindromic",
    factorial: "Factorial",
    primorial: "Primorial",
    twin: "Twin",
    sophie_germain: "Sophie Germain",
    cullen_woodall: "Cullen/Woodall",
    carol_kynea: "Carol/Kynea",
    gen_fermat: "Gen. Fermat",
    repunit: "Repunit",
    wagstaff: "Wagstaff",
    near_repdigit: "Near-Repdigit",
  };
  return labels[form] ?? form;
}

function formColor(form: string): string {
  const colors: Record<string, string> = {
    kbn: "bg-indigo-500/10 text-indigo-400 border-indigo-500/20",
    palindromic: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
    factorial: "bg-amber-500/10 text-amber-400 border-amber-500/20",
    primorial: "bg-violet-500/10 text-violet-400 border-violet-500/20",
    twin: "bg-cyan-500/10 text-cyan-400 border-cyan-500/20",
    sophie_germain: "bg-rose-500/10 text-rose-400 border-rose-500/20",
    cullen_woodall: "bg-orange-500/10 text-orange-400 border-orange-500/20",
    carol_kynea: "bg-pink-500/10 text-pink-400 border-pink-500/20",
    gen_fermat: "bg-teal-500/10 text-teal-400 border-teal-500/20",
    repunit: "bg-sky-500/10 text-sky-400 border-sky-500/20",
    wagstaff: "bg-purple-500/10 text-purple-400 border-purple-500/20",
    near_repdigit: "bg-lime-500/10 text-lime-400 border-lime-500/20",
  };
  return colors[form] ?? "bg-accent-purple/10 text-accent-purple border-accent-purple/20";
}

export function LiveFeed() {
  const [entries, setEntries] = useState<PrimeEntry[]>(FALLBACK_ENTRIES);
  const [live, setLive] = useState(false);

  useEffect(() => {
    let active = true;
    async function fetchRecent() {
      try {
        const res = await fetch(`${API_BASE}/api/status`);
        if (!res.ok) return;
        const data = (await res.json()) as { recent_primes?: PrimeEntry[] };
        if (active && data.recent_primes && data.recent_primes.length > 0) {
          setEntries(data.recent_primes.slice(0, 6));
          setLive(true);
        }
      } catch {
        // Keep fallback
      }
    }
    fetchRecent();
    const timer = setInterval(fetchRecent, 30000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, []);

  return (
    <Section>
      <div className="flex items-center justify-between mb-8">
        <div>
          <div className="flex items-center gap-3 mb-1">
            <h2 className="text-3xl sm:text-4xl font-bold text-foreground">Discoveries</h2>
            {live && (
              <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium bg-accent-green/10 text-accent-green border border-accent-green/20">
                <span className="inline-block w-1.5 h-1.5 rounded-full bg-accent-green pulse-green" />
                Live
              </span>
            )}
          </div>
          <p className="text-muted-foreground text-sm sm:text-base">
            Latest primes found by the network, updated in real time.
          </p>
        </div>
        <a
          href="https://app.darkreach.ai/browse"
          className="hidden sm:inline-flex items-center gap-1.5 text-sm text-primary hover:underline flex-shrink-0"
        >
          View all
          <ExternalLink size={13} />
        </a>
      </div>

      {/* Latest discovery — featured */}
      {entries.length > 0 && (
        <div className="mb-3 p-5 rounded-xl border border-accent-purple/20 bg-accent-purple/[0.03]">
          <div className="flex items-center gap-2 mb-3">
            <span className="text-[11px] font-medium text-accent-purple/70 uppercase tracking-wider">Latest</span>
            <span className="text-xs text-muted-foreground/50">{timeAgo(entries[0].discovered_at)}</span>
          </div>
          <div className="flex items-center gap-3 flex-wrap">
            <span className={`inline-flex px-2.5 py-1 rounded-md text-[11px] font-medium border flex-shrink-0 ${formColor(entries[0].form)}`}>
              {formLabel(entries[0].form)}
            </span>
            <span className="font-mono text-base sm:text-lg text-foreground truncate flex-1 min-w-0">
              {entries[0].expression}
            </span>
            <span className="text-sm font-mono text-muted-foreground flex-shrink-0 tabular-nums">
              {entries[0].digits.toLocaleString()}
              <span className="text-muted-foreground/60 ml-0.5">digits</span>
            </span>
          </div>
        </div>
      )}

      {/* Remaining discoveries */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        {entries.slice(1).map((entry) => (
          <div
            key={entry.id}
            className="flex items-center gap-3 px-4 py-3 rounded-lg border border-border/60 hover:border-border hover:bg-card/40 transition-colors"
          >
            <span className={`inline-flex px-2.5 py-1 rounded-md text-[11px] font-medium border flex-shrink-0 ${formColor(entry.form)}`}>
              {formLabel(entry.form)}
            </span>
            <span className="font-mono text-sm text-foreground truncate flex-1 min-w-0">
              {entry.expression}
            </span>
            <span className="text-sm font-mono text-muted-foreground flex-shrink-0 tabular-nums">
              {entry.digits.toLocaleString()}
              <span className="text-muted-foreground/60 ml-0.5">d</span>
            </span>
            <span className="text-xs text-muted-foreground/60 flex-shrink-0 w-14 text-right tabular-nums">
              {timeAgo(entry.discovered_at)}
            </span>
          </div>
        ))}
      </div>

      <div className="mt-4 text-center sm:hidden">
        <a
          href="https://app.darkreach.ai/browse"
          className="text-sm text-primary hover:underline"
        >
          View all discoveries →
        </a>
      </div>
    </Section>
  );
}
