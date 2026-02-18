"use client";

/**
 * @module new-search-dialog
 *
 * Modal dialog for launching a new prime search. Presents a form with
 * all 12 supported prime forms (factorial, kbn, palindromic, etc.),
 * each with their specific parameter fields. Submits to the Rust
 * backend's `/api/searches/start` endpoint. Includes algorithm
 * descriptions and parameter validation.
 */

import { useMemo, useState } from "react";
import { API_BASE } from "@/lib/format";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Info } from "lucide-react";

interface NewSearchDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: () => void;
}

type SearchType =
  | "kbn"
  | "factorial"
  | "palindromic"
  | "primorial"
  | "cullen_woodall"
  | "wagstaff"
  | "carol_kynea"
  | "twin"
  | "sophie_germain"
  | "repunit"
  | "gen_fermat";

interface FormMeta {
  label: string;
  description: string;
  hint: string;
  proof: string;
  fields: { key: string; label: string; placeholder?: string }[];
  defaults: Record<string, number>;
}

const formMeta: Record<SearchType, FormMeta> = {
  kbn: {
    label: "KBN (k*b^n +/- 1)",
    description: "General Proth/Riesel form. Tests both +1 and -1.",
    hint: "Uses Proth test (base 2, +1), LLR test (-1), or BSGS sieve + MR fallback. Deterministic proofs via Pocklington/Morrison when applicable.",
    proof: "Deterministic (Proth/LLR/Pocklington)",
    fields: [
      { key: "k", label: "k", placeholder: "3" },
      { key: "base", label: "Base", placeholder: "2" },
      { key: "min_n", label: "Min n", placeholder: "1" },
      { key: "max_n", label: "Max n", placeholder: "100000" },
    ],
    defaults: { k: 3, base: 2, min_n: 1, max_n: 100000 },
  },
  factorial: {
    label: "Factorial (n! +/- 1)",
    description: "Tests n!+1 and n!-1 for each n in range.",
    hint: "Incremental factorial computation. Pocklington proof for n!+1, Morrison proof for n!-1. Both +1 and -1 tested in parallel.",
    proof: "Deterministic (Pocklington/Morrison)",
    fields: [
      { key: "start", label: "Start n", placeholder: "1" },
      { key: "end", label: "End n", placeholder: "10000" },
    ],
    defaults: { start: 1, end: 10000 },
  },
  palindromic: {
    label: "Palindromic",
    description: "Palindromic primes in a given base. Even-digit palindromes auto-skipped.",
    hint: "Generates half-values and mirrors them. Deep modular sieve eliminates most composites before GMP testing.",
    proof: "Probabilistic (Miller-Rabin)",
    fields: [
      { key: "base", label: "Base", placeholder: "10" },
      { key: "min_digits", label: "Min digits", placeholder: "1" },
      { key: "max_digits", label: "Max digits", placeholder: "11" },
    ],
    defaults: { base: 10, min_digits: 1, max_digits: 11 },
  },
  primorial: {
    label: "Primorial (p# +/- 1)",
    description: "Tests p#+1 and p#-1 where p# is the product of all primes up to p.",
    hint: "Same proof strategy as factorial (shared prime factorization). Extremely rare finds — only 7 known for each sign.",
    proof: "Deterministic (Pocklington/Morrison)",
    fields: [
      { key: "start", label: "Start prime", placeholder: "2" },
      { key: "end", label: "End prime", placeholder: "10000" },
    ],
    defaults: { start: 2, end: 10000 },
  },
  cullen_woodall: {
    label: "Cullen/Woodall",
    description: "Cullen: n*2^n+1, Woodall: n*2^n-1. Both tested per n.",
    hint: "Uses incremental multiplication. LLR-provable since k = n. Carol/Kynea forms also decompose to this pattern.",
    proof: "Deterministic (LLR)",
    fields: [
      { key: "min_n", label: "Min n", placeholder: "1" },
      { key: "max_n", label: "Max n", placeholder: "100000" },
    ],
    defaults: { min_n: 1, max_n: 100000 },
  },
  wagstaff: {
    label: "Wagstaff",
    description: "Wagstaff numbers: (2^p+1)/3 for prime p.",
    hint: "No deterministic proof exists — results are always probable primes (PRP). No active competing project. Only 44 known Wagstaff primes.",
    proof: "Probabilistic (PRP only)",
    fields: [
      { key: "min_exp", label: "Min exponent", placeholder: "3" },
      { key: "max_exp", label: "Max exponent", placeholder: "100000" },
    ],
    defaults: { min_exp: 3, max_exp: 100000 },
  },
  carol_kynea: {
    label: "Carol/Kynea",
    description: "Carol: (2^n-1)^2-2, Kynea: (2^n+1)^2-2.",
    hint: "Decompose to k*2^(n+1)-1 form (Carol: k=2^(n-1)-1, Kynea: k=2^(n-1)+1). LLR-provable.",
    proof: "Deterministic (LLR)",
    fields: [
      { key: "min_n", label: "Min n", placeholder: "2" },
      { key: "max_n", label: "Max n", placeholder: "100000" },
    ],
    defaults: { min_n: 2, max_n: 100000 },
  },
  twin: {
    label: "Twin Primes",
    description: "Twin primes of form k*b^n+1 and k*b^n-1 (both must be prime).",
    hint: "Intersects BSGS sieve survivors for +1 and -1. Both candidates tested only if both survive the sieve.",
    proof: "Deterministic (Proth + LLR)",
    fields: [
      { key: "k", label: "k", placeholder: "1" },
      { key: "base", label: "Base", placeholder: "2" },
      { key: "min_n", label: "Min n", placeholder: "1" },
      { key: "max_n", label: "Max n", placeholder: "100000" },
    ],
    defaults: { k: 1, base: 2, min_n: 1, max_n: 100000 },
  },
  sophie_germain: {
    label: "Sophie Germain",
    description: "Sophie Germain: p=k*b^n-1 where both p and 2p+1 are prime.",
    hint: "Reuses KBN sieve: p=k*b^n-1, safe prime=2k*b^n-1. Both are LLR-testable.",
    proof: "Deterministic (LLR)",
    fields: [
      { key: "k", label: "k", placeholder: "1" },
      { key: "base", label: "Base", placeholder: "2" },
      { key: "min_n", label: "Min n", placeholder: "1" },
      { key: "max_n", label: "Max n", placeholder: "100000" },
    ],
    defaults: { k: 1, base: 2, min_n: 1, max_n: 100000 },
  },
  repunit: {
    label: "Repunit",
    description: "Repunit primes: R(b,n) = (b^n-1)/(b-1) for prime n.",
    hint: "Each sieve prime eliminates at most one exponent. Only prime exponents tested. Probabilistic results (no known proof method).",
    proof: "Probabilistic (Miller-Rabin)",
    fields: [
      { key: "base", label: "Base", placeholder: "10" },
      { key: "min_n", label: "Min n", placeholder: "2" },
      { key: "max_n", label: "Max n", placeholder: "100000" },
    ],
    defaults: { base: 10, min_n: 2, max_n: 100000 },
  },
  gen_fermat: {
    label: "Generalized Fermat",
    description: "Generalized Fermat primes: b^(2^n)+1 for even b.",
    hint: "Deterministic Pepin/Proth proof when 2^t > m (b = 2^t * m, m odd). Scans base range for a fixed Fermat exponent.",
    proof: "Deterministic (Pepin/Proth when applicable)",
    fields: [
      { key: "fermat_exp", label: "Fermat exponent n", placeholder: "1" },
      { key: "min_base", label: "Min base (even)", placeholder: "2" },
      { key: "max_base", label: "Max base", placeholder: "1000000" },
    ],
    defaults: { fermat_exp: 1, min_base: 2, max_base: 1000000 },
  },
};

interface Preset {
  name: string;
  description: string;
  type: SearchType;
  values: Record<string, number>;
  tag?: string;
}

const presets: Preset[] = [
  {
    name: "Proth primes (small)",
    description: "3*2^n+1, quick scan up to n=100k",
    tag: "Fast",
    type: "kbn",
    values: { k: 3, base: 2, min_n: 1, max_n: 100000 },
  },
  {
    name: "Proth primes (deep)",
    description: "3*2^n+1, extended range to 1M",
    tag: "Long",
    type: "kbn",
    values: { k: 3, base: 2, min_n: 100000, max_n: 1000000 },
  },
  {
    name: "Riesel primes",
    description: "k*2^n-1 with k=21, Riesel form",
    type: "kbn",
    values: { k: 21, base: 2, min_n: 1, max_n: 500000 },
  },
  {
    name: "Base-3 search",
    description: "5*3^n+1, less explored territory",
    tag: "Novel",
    type: "kbn",
    values: { k: 5, base: 3, min_n: 1, max_n: 200000 },
  },
  {
    name: "Factorial (quick)",
    description: "n!+/-1 from 1 to 1,000",
    tag: "Fast",
    type: "factorial",
    values: { start: 1, end: 1000 },
  },
  {
    name: "Factorial (extended)",
    description: "n!+/-1 from 1,000 to 10,000",
    tag: "Long",
    type: "factorial",
    values: { start: 1000, end: 10000 },
  },
  {
    name: "Palindromic (base 10)",
    description: "Decimal palindromes, 1-11 digits",
    tag: "Fast",
    type: "palindromic",
    values: { base: 10, min_digits: 1, max_digits: 11 },
  },
  {
    name: "Twin primes",
    description: "Twin primes k*2^n +/- 1, n up to 100k",
    tag: "Novel",
    type: "twin",
    values: { k: 1, base: 2, min_n: 1, max_n: 100000 },
  },
  {
    name: "Wagstaff (small)",
    description: "(2^p+1)/3 for prime p up to 100k",
    type: "wagstaff",
    values: { min_exp: 3, max_exp: 100000 },
  },
  {
    name: "Cullen/Woodall",
    description: "n*2^n+1 and n*2^n-1 up to n=50k",
    type: "cullen_woodall",
    values: { min_n: 1, max_n: 50000 },
  },
  {
    name: "Sophie Germain",
    description: "p and 2p+1 both prime, k*2^n-1 form",
    tag: "Novel",
    type: "sophie_germain",
    values: { k: 1, base: 2, min_n: 1, max_n: 100000 },
  },
  {
    name: "Gen Fermat (n=1)",
    description: "b^2+1 for even b up to 1M",
    tag: "Fast",
    type: "gen_fermat",
    values: { fermat_exp: 1, min_base: 2, max_base: 1000000 },
  },
  {
    name: "Repunit (base 10)",
    description: "(10^n-1)/9 for prime n up to 100k",
    type: "repunit",
    values: { base: 10, min_n: 2, max_n: 100000 },
  },
];

const tagColors: Record<string, string> = {
  Fast: "bg-green-500/15 text-green-500 border-green-500/30",
  Long: "bg-yellow-500/15 text-yellow-500 border-yellow-500/30",
  Heavy: "bg-red-500/15 text-red-500 border-red-500/30",
  Novel: "bg-blue-500/15 text-blue-500 border-blue-500/30",
};

export function NewSearchDialog({ open, onOpenChange, onCreated }: NewSearchDialogProps) {
  const [mode, setMode] = useState<"presets" | "custom">("presets");
  const [searchType, setSearchType] = useState<SearchType>("kbn");
  const [values, setValues] = useState<Record<string, number>>(formMeta.kbn.defaults);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const meta = formMeta[searchType];

  const algorithmHints = useMemo(() => {
    const m = formMeta[searchType];
    const hints: string[] = [];

    if (searchType === "kbn" && values.base === 2) {
      if (values.k && values.k % 2 === 1) {
        hints.push("Proth test available for +1 side (deterministic, single exponentiation)");
      }
      hints.push("LLR test available for -1 side (deterministic, n-2 squarings)");
    }

    if (searchType === "kbn" && values.base !== 2) {
      hints.push("Non-base-2: BSGS sieve + Miller-Rabin fallback. Less explored territory — higher discovery probability.");
    }

    if (searchType === "wagstaff") {
      hints.push("No deterministic proof exists for Wagstaff primes. All results are PRP.");
    }

    if (searchType === "gen_fermat" && values.fermat_exp >= 10) {
      hints.push("Large Fermat exponent: candidates grow as b^(2^n). Expect very large numbers.");
    }

    if (m.proof.startsWith("Deterministic")) {
      hints.push(`Proof: ${m.proof} — results publishable to Top5000 / OEIS.`);
    }

    return hints;
  }, [searchType, values.base, values.k, values.fermat_exp]);

  function handleTypeChange(type: SearchType) {
    setSearchType(type);
    setValues(formMeta[type].defaults);
    setError(null);
  }

  function handleValueChange(key: string, raw: string) {
    const num = parseInt(raw, 10);
    if (!isNaN(num)) {
      setValues((prev) => ({ ...prev, [key]: num }));
    }
  }

  function applyPreset(preset: Preset) {
    setSearchType(preset.type);
    setValues(preset.values);
    setMode("custom");
    setError(null);
  }

  async function handleSubmit() {
    setError(null);
    setSubmitting(true);
    try {
      const body = { search_type: searchType, ...values };
      const res = await fetch(`${API_BASE}/api/searches`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      onOpenChange(false);
      onCreated();
      setMode("presets");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to start search");
    } finally {
      setSubmitting(false);
    }
  }

  function handleOpenChange(v: boolean) {
    if (!v) setMode("presets");
    onOpenChange(v);
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>New Search</DialogTitle>
        </DialogHeader>

        {mode === "presets" ? (
          <div className="space-y-3">
            <p className="text-sm text-muted-foreground">
              Pick a recommended config or build your own.
            </p>
            <div className="grid gap-2 max-h-[360px] overflow-y-auto pr-1">
              {presets.map((p) => (
                <button
                  key={p.name}
                  onClick={() => applyPreset(p)}
                  className="text-left rounded-lg border border-border p-3 hover:bg-secondary/50 transition-colors"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-foreground">{p.name}</span>
                    <Badge variant="outline" className="text-xs px-1.5 py-0">
                      {p.type}
                    </Badge>
                    {p.tag && (
                      <span className={`text-xs px-1.5 py-0 rounded-full border ${tagColors[p.tag] || ""}`}>
                        {p.tag}
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-muted-foreground mt-1">{p.description}</div>
                </button>
              ))}
            </div>
            <Button variant="outline" className="w-full" onClick={() => setMode("custom")}>
              Custom configuration
            </Button>
          </div>
        ) : (
          <div className="space-y-4">
            <div>
              <label className="text-xs font-medium text-muted-foreground mb-1 block">
                Search type
              </label>
              <Select value={searchType} onValueChange={(v) => handleTypeChange(v as SearchType)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {(Object.entries(formMeta) as [SearchType, FormMeta][]).map(([key, m]) => (
                    <SelectItem key={key} value={key}>
                      {m.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground mt-1.5">{meta.description}</p>
            </div>

            <div className={meta.fields.length <= 2 ? "grid grid-cols-2 gap-3" : "grid grid-cols-2 gap-3"}>
              {meta.fields.map((field) => (
                <div key={field.key}>
                  <label className="text-xs font-medium text-muted-foreground mb-1 block">
                    {field.label}
                  </label>
                  <Input
                    type="number"
                    value={values[field.key] ?? ""}
                    placeholder={field.placeholder}
                    onChange={(e) => handleValueChange(field.key, e.target.value)}
                  />
                </div>
              ))}
            </div>

            {algorithmHints.length > 0 && (
              <div className="rounded-md bg-muted p-3 space-y-1">
                <div className="flex items-center gap-1.5 text-xs font-medium text-foreground">
                  <Info className="size-3.5" />
                  Algorithm info
                </div>
                {algorithmHints.map((hint, i) => (
                  <p key={i} className="text-xs text-muted-foreground">
                    {hint}
                  </p>
                ))}
              </div>
            )}

            {error && (
              <div className="text-sm text-red-500 bg-red-500/10 rounded-md px-3 py-2">
                {error}
              </div>
            )}

            <div className="flex gap-2">
              <Button variant="outline" className="flex-1" onClick={() => setMode("presets")}>
                Back
              </Button>
              <Button onClick={handleSubmit} disabled={submitting} className="flex-1">
                {submitting ? "Starting..." : "Start Search"}
              </Button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
