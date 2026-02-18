"use client";

import { useState } from "react";
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

interface NewSearchDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: () => void;
}

type SearchType = "kbn" | "factorial" | "palindromic";

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
    name: "Factorial (frontier)",
    description: "n!+/-1 from 10k to 100k — large candidates",
    tag: "Heavy",
    type: "factorial",
    values: { start: 10000, end: 100000 },
  },
  {
    name: "Palindromic (base 10)",
    description: "Decimal palindromes, 1-11 digits",
    tag: "Fast",
    type: "palindromic",
    values: { base: 10, min_digits: 1, max_digits: 11 },
  },
  {
    name: "Palindromic (large)",
    description: "Decimal palindromes, 11-17 digits",
    tag: "Long",
    type: "palindromic",
    values: { base: 10, min_digits: 11, max_digits: 17 },
  },
  {
    name: "Palindromic (base 2)",
    description: "Binary palindromes, 1-31 digits",
    type: "palindromic",
    values: { base: 2, min_digits: 1, max_digits: 31 },
  },
];

const defaults: Record<SearchType, Record<string, number>> = {
  kbn: { k: 3, base: 2, min_n: 1, max_n: 100000 },
  factorial: { start: 1, end: 10000 },
  palindromic: { base: 10, min_digits: 1, max_digits: 11 },
};

const tagColors: Record<string, string> = {
  Fast: "bg-green-500/15 text-green-500 border-green-500/30",
  Long: "bg-yellow-500/15 text-yellow-500 border-yellow-500/30",
  Heavy: "bg-red-500/15 text-red-500 border-red-500/30",
  Novel: "bg-blue-500/15 text-blue-500 border-blue-500/30",
};

export function NewSearchDialog({ open, onOpenChange, onCreated }: NewSearchDialogProps) {
  const [mode, setMode] = useState<"presets" | "custom">("presets");
  const [searchType, setSearchType] = useState<SearchType>("kbn");
  const [values, setValues] = useState<Record<string, number>>(defaults.kbn);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  function handleTypeChange(type: SearchType) {
    setSearchType(type);
    setValues(defaults[type]);
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
                  <SelectItem value="kbn">KBN (k*b^n +/- 1)</SelectItem>
                  <SelectItem value="factorial">Factorial (n! +/- 1)</SelectItem>
                  <SelectItem value="palindromic">Palindromic</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {searchType === "kbn" && (
              <>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">k</label>
                    <Input type="number" value={values.k} onChange={(e) => handleValueChange("k", e.target.value)} />
                  </div>
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Base</label>
                    <Input type="number" value={values.base} onChange={(e) => handleValueChange("base", e.target.value)} />
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Min n</label>
                    <Input type="number" value={values.min_n} onChange={(e) => handleValueChange("min_n", e.target.value)} />
                  </div>
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Max n</label>
                    <Input type="number" value={values.max_n} onChange={(e) => handleValueChange("max_n", e.target.value)} />
                  </div>
                </div>
              </>
            )}

            {searchType === "factorial" && (
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="text-xs font-medium text-muted-foreground mb-1 block">Start</label>
                  <Input type="number" value={values.start} onChange={(e) => handleValueChange("start", e.target.value)} />
                </div>
                <div>
                  <label className="text-xs font-medium text-muted-foreground mb-1 block">End</label>
                  <Input type="number" value={values.end} onChange={(e) => handleValueChange("end", e.target.value)} />
                </div>
              </div>
            )}

            {searchType === "palindromic" && (
              <>
                <div>
                  <label className="text-xs font-medium text-muted-foreground mb-1 block">Base</label>
                  <Input type="number" value={values.base} onChange={(e) => handleValueChange("base", e.target.value)} />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Min digits</label>
                    <Input type="number" value={values.min_digits} onChange={(e) => handleValueChange("min_digits", e.target.value)} />
                  </div>
                  <div>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">Max digits</label>
                    <Input type="number" value={values.max_digits} onChange={(e) => handleValueChange("max_digits", e.target.value)} />
                  </div>
                </div>
              </>
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
