export const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export function numberWithCommas(x: number): string {
  return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

export function formToSlug(form: string): string {
  const map: Record<string, string> = {
    Factorial: "factorial",
    Palindromic: "palindromic",
    Kbn: "kbn",
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
