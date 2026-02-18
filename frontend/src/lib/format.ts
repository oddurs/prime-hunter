/**
 * @module format
 *
 * Shared formatting utilities used across the dashboard. Includes:
 *
 * - `API_BASE` — Base URL for the Rust backend REST API
 * - `numberWithCommas()` — Locale-style number formatting (e.g., 1,234,567)
 * - `formToSlug()` — Normalizes prime form names to URL-safe slugs
 * - `formLabels` — Human-readable labels for each prime form
 * - `relativeTime()` — "3 minutes ago" style timestamp formatting
 * - `formatTime()` / `formatUptime()` — Absolute time and duration formatting
 */

export const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export function numberWithCommas(x: number): string {
  return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

export function formToSlug(form: string): string {
  const map: Record<string, string> = {
    Factorial: "factorial",
    Palindromic: "palindromic",
    Kbn: "kbn",
    KBN: "kbn",
    factorial: "factorial",
    palindromic: "palindromic",
    kbn: "kbn",
    primorial: "new-forms",
    cullen: "new-forms",
    woodall: "new-forms",
    cullen_woodall: "new-forms",
    wagstaff: "new-forms",
    carol: "new-forms",
    kynea: "new-forms",
    carol_kynea: "new-forms",
    twin: "new-forms",
    sophie_germain: "new-forms",
    repunit: "new-forms",
    gen_fermat: "new-forms",
    near_repdigit: "new-forms",
  };
  return map[form] || form.toLowerCase();
}

export function formatTime(iso: string): string {
  return new Date(iso).toLocaleString();
}

export function formatUptime(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** Human-readable labels for each prime form, shared across components. */
export const formLabels: Record<string, string> = {
  factorial: "Factorial",
  primorial: "Primorial",
  wagstaff: "Wagstaff",
  palindromic: "Palindromic",
  twin: "Twin",
  sophie_germain: "Sophie Germain",
  repunit: "Repunit",
  gen_fermat: "Gen. Fermat",
  kbn: "k·b^n",
  cullen_woodall: "Cullen/Woodall",
  carol_kynea: "Carol/Kynea",
  near_repdigit: "Near-repdigit",
};

/**
 * Formats an ISO timestamp as a human-readable relative time string.
 * Produces compact output: "just now", "3m ago", "2h 15m ago", "5d ago", "2mo ago".
 */
export function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ${mins % 60}m ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return `${Math.floor(days / 30)}mo ago`;
}
