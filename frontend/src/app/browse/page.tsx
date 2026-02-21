"use client";

/**
 * @module browse/page
 *
 * Infinite-scroll prime browser with a polished list view. Features:
 *
 * - **Infinite scroll**: IntersectionObserver pre-fetches 400px before sentinel
 * - **Sticky filter bar**: search, form, digit range, sort — all URL-synced
 * - **Active filter pills**: dismissable badges showing current filters
 * - **List rows**: verification dot, expression, form badge, digits, relative time
 * - **Detail dialog**: click a row to see full prime details + verify
 * - **Loading states**: skeleton rows, end-of-results, empty state
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { ChevronRight, Download, Search, SearchX, X } from "lucide-react";
import { usePrimes, type PrimeFilter, type PrimeRecord } from "@/hooks/use-primes";
import { useStats } from "@/hooks/use-stats";
import {
  API_BASE,
  formLabels,
  formToSlug,
  formatTime,
  numberWithCommas,
  relativeTime,
} from "@/lib/format";
import { cn } from "@/lib/utils";
import { ViewHeader } from "@/components/view-header";
import { PrimeDetailDialog } from "@/components/prime-detail-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

const SORT_OPTIONS: Record<string, string> = {
  "found_at:desc": "Newest first",
  "found_at:asc": "Oldest first",
  "digits:desc": "Most digits",
  "digits:asc": "Fewest digits",
};

function parsePositiveInteger(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return 0;
  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 1) return null;
  return parsed;
}

function SkeletonRow() {
  return (
    <div className="flex items-center gap-3 px-4 py-3 border-b last:border-b-0">
      <Skeleton className="size-2 rounded-full shrink-0" />
      <div className="flex-1 min-w-0 space-y-2">
        <Skeleton className="h-4 w-48" />
        <div className="flex items-center gap-2">
          <Skeleton className="h-5 w-16 rounded-full" />
          <Skeleton className="h-3 w-20" />
          <Skeleton className="h-3 w-14" />
        </div>
      </div>
      <Skeleton className="size-4 shrink-0" />
    </div>
  );
}

function PrimeRow({
  prime,
  isActive,
  onClick,
}: {
  prime: PrimeRecord;
  isActive: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "group flex items-center gap-3 w-full text-left px-4 py-3 border-b last:border-b-0",
        "transition-colors duration-100 hover:bg-muted/50 focus-visible:bg-muted/50 focus-visible:outline-none",
        isActive && "bg-muted/40"
      )}
    >
      {/* Verification dot */}
      <span
        className={cn(
          "size-2 rounded-full shrink-0",
          prime.verified ? "bg-green-500" : "bg-muted-foreground/30"
        )}
      />

      {/* Content */}
      <div className="flex-1 min-w-0">
        <div className="font-mono text-sm text-primary truncate">
          {prime.expression}
        </div>
        <div className="flex items-center gap-2 mt-0.5">
          <Link
            href={`/docs?doc=${formToSlug(prime.form)}`}
            onClick={(e) => e.stopPropagation()}
          >
            <Badge
              variant="outline"
              className="cursor-pointer hover:bg-secondary/50 text-[11px] px-1.5 py-0"
            >
              {formLabels[prime.form] ?? prime.form}
            </Badge>
          </Link>
          <span className="text-xs text-muted-foreground">
            {numberWithCommas(prime.digits)} digits
          </span>
          <span className="text-xs text-muted-foreground/60 hidden sm:inline">
            &middot;
          </span>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="text-xs text-muted-foreground hidden sm:inline">
                  {relativeTime(prime.found_at)}
                </span>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                {formatTime(prime.found_at)}
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </div>

      {/* Chevron affordance */}
      <ChevronRight className="size-4 text-muted-foreground/30 group-hover:text-muted-foreground/70 transition-colors shrink-0" />
    </button>
  );
}

export default function BrowsePage() {
  const { stats } = useStats();
  const {
    primes,
    selectedPrime,
    fetchPrimeDetail,
    clearSelectedPrime,
    resetAndFetch,
    fetchNextPage,
    hasMore,
    isLoadingMore,
    isInitialLoading,
  } = usePrimes();

  const [searchInput, setSearchInput] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [formFilter, setFormFilter] = useState("");
  const [minDigits, setMinDigits] = useState("");
  const [maxDigits, setMaxDigits] = useState("");
  const [sortKey, setSortKey] = useState("found_at:desc");
  const [detailOpen, setDetailOpen] = useState(false);
  const [pendingPrimeId, setPendingPrimeId] = useState<number | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [initialized, setInitialized] = useState(false);

  const sentinelRef = useRef<HTMLDivElement>(null);
  const total = primes.total;

  // Available form names from stats + loaded primes
  const forms = useMemo(() => {
    const fromStats = stats?.by_form?.map((f) => f.form) ?? [];
    const fromPrimes = primes.primes.map((p) => p.form);
    return Array.from(new Set([...fromStats, ...fromPrimes])).sort();
  }, [stats?.by_form, primes.primes]);

  // Debounce search input
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(searchInput), 300);
    return () => clearTimeout(timer);
  }, [searchInput]);

  // Parse URL params on mount
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const q = params.get("q");
    const form = params.get("form");
    const min = params.get("min_digits");
    const max = params.get("max_digits");
    const sortBy = params.get("sort_by");
    const sortDir = params.get("sort_dir");
    const prime = params.get("prime");

    if (q) { setSearchInput(q); setDebouncedSearch(q); }
    if (form) setFormFilter(form);
    if (min) setMinDigits(min);
    if (max) setMaxDigits(max);
    if (sortBy) setSortKey(`${sortBy}:${sortDir || "desc"}`);
    if (prime) {
      const id = Number(prime);
      if (Number.isInteger(id) && id > 0) {
        setPendingPrimeId(id);
        setDetailOpen(true);
      }
    }
    setInitialized(true);
  }, []);

  // Sync state to URL
  useEffect(() => {
    if (!initialized) return;
    const params = new URLSearchParams();
    if (debouncedSearch) params.set("q", debouncedSearch);
    if (formFilter) params.set("form", formFilter);
    if (minDigits.trim()) params.set("min_digits", minDigits.trim());
    if (maxDigits.trim()) params.set("max_digits", maxDigits.trim());
    const [sortBy, sortDir] = sortKey.split(":");
    if (sortBy !== "found_at" || sortDir !== "desc") {
      params.set("sort_by", sortBy);
      params.set("sort_dir", sortDir);
    }
    if (detailOpen && pendingPrimeId !== null) {
      params.set("prime", String(pendingPrimeId));
    }
    const query = params.toString();
    window.history.replaceState({}, "", query ? `/browse?${query}` : "/browse");
  }, [debouncedSearch, formFilter, minDigits, maxDigits, sortKey, detailOpen, pendingPrimeId, initialized]);

  // Digit validation
  const parsedMinDigits = useMemo(() => parsePositiveInteger(minDigits), [minDigits]);
  const parsedMaxDigits = useMemo(() => parsePositiveInteger(maxDigits), [maxDigits]);
  const digitsError = useMemo(() => {
    if (parsedMinDigits === null || parsedMaxDigits === null)
      return "Digit filters must be positive integers.";
    if (parsedMinDigits > 0 && parsedMaxDigits > 0 && parsedMinDigits > parsedMaxDigits)
      return "Min digits cannot be greater than max digits.";
    return null;
  }, [parsedMinDigits, parsedMaxDigits]);

  // Build filter from current state
  const buildFilter = useCallback((): PrimeFilter => {
    const [sortBy, sortDir] = sortKey.split(":");
    const f: PrimeFilter = { sort_by: sortBy, sort_dir: sortDir };
    if (formFilter) f.form = formFilter;
    if (debouncedSearch) f.search = debouncedSearch;
    if (parsedMinDigits && parsedMinDigits > 0) f.min_digits = parsedMinDigits;
    if (parsedMaxDigits && parsedMaxDigits > 0) f.max_digits = parsedMaxDigits;
    return f;
  }, [formFilter, debouncedSearch, parsedMinDigits, parsedMaxDigits, sortKey]);

  // Fetch on filter change
  useEffect(() => {
    if (!initialized || digitsError) return;
    resetAndFetch(buildFilter());
  }, [debouncedSearch, formFilter, minDigits, maxDigits, sortKey, digitsError, buildFilter, resetAndFetch, initialized]);

  // IntersectionObserver for infinite scroll
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && hasMore && !isLoadingMore && !isInitialLoading) {
          fetchNextPage();
        }
      },
      { rootMargin: "0px 0px 400px 0px" }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, isLoadingMore, isInitialLoading, fetchNextPage]);

  // Prime detail
  useEffect(() => {
    if (pendingPrimeId === null || !detailOpen) return;
    clearSelectedPrime();
    setDetailLoading(true);
    fetchPrimeDetail(pendingPrimeId);
  }, [pendingPrimeId, detailOpen, fetchPrimeDetail, clearSelectedPrime]);

  useEffect(() => {
    if (!selectedPrime || pendingPrimeId === null) return;
    if (selectedPrime.id === pendingPrimeId) setDetailLoading(false);
  }, [selectedPrime, pendingPrimeId]);

  // Active filter checks
  const hasActiveFilters = !!(
    formFilter ||
    debouncedSearch ||
    minDigits ||
    maxDigits ||
    sortKey !== "found_at:desc"
  );

  function clearFilters() {
    setSearchInput("");
    setDebouncedSearch("");
    setFormFilter("");
    setMinDigits("");
    setMaxDigits("");
    setSortKey("found_at:desc");
  }

  function handleRowClick(id: number) {
    setPendingPrimeId(id);
    setDetailLoading(true);
    setDetailOpen(true);
  }

  function handleDetailClose(open: boolean) {
    if (!open) {
      setDetailOpen(false);
      setPendingPrimeId(null);
      setDetailLoading(false);
      clearSelectedPrime();
    }
  }

  function exportData(format: "csv" | "json") {
    if (digitsError) return;
    const params = new URLSearchParams();
    params.set("format", format);
    if (formFilter) params.set("form", formFilter);
    if (debouncedSearch) params.set("search", debouncedSearch);
    if (parsedMinDigits && parsedMinDigits > 0) params.set("min_digits", String(parsedMinDigits));
    if (parsedMaxDigits && parsedMaxDigits > 0) params.set("max_digits", String(parsedMaxDigits));
    const [sortBy, sortDir] = sortKey.split(":");
    params.set("sort_by", sortBy);
    params.set("sort_dir", sortDir);
    window.open(`${API_BASE}/api/export?${params.toString()}`, "_blank");
  }

  // Build filter pill data
  const filterPills: { key: string; label: string; onClear: () => void }[] = [];
  if (debouncedSearch) {
    filterPills.push({
      key: "search",
      label: `"${debouncedSearch}"`,
      onClear: () => { setSearchInput(""); setDebouncedSearch(""); },
    });
  }
  if (formFilter) {
    filterPills.push({
      key: "form",
      label: formLabels[formFilter] ?? formFilter,
      onClear: () => setFormFilter(""),
    });
  }
  if (minDigits) {
    filterPills.push({
      key: "min",
      label: `${"\u2265"} ${numberWithCommas(Number(minDigits))} digits`,
      onClear: () => setMinDigits(""),
    });
  }
  if (maxDigits) {
    filterPills.push({
      key: "max",
      label: `${"\u2264"} ${numberWithCommas(Number(maxDigits))} digits`,
      onClear: () => setMaxDigits(""),
    });
  }
  if (sortKey !== "found_at:desc") {
    filterPills.push({
      key: "sort",
      label: SORT_OPTIONS[sortKey],
      onClear: () => setSortKey("found_at:desc"),
    });
  }

  return (
    <>
      <ViewHeader
        title="Browse"
        subtitle={
          total === 0 && !isInitialLoading
            ? "No primes yet"
            : `${numberWithCommas(total)} primes in the archive`
        }
        actions={
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm">
                <Download className="size-3.5 mr-1.5" />
                Export
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={() => exportData("csv")} disabled={!!digitsError}>
                Export CSV
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => exportData("json")} disabled={!!digitsError}>
                Export JSON
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        }
        className="mb-0"
      />

      {/* Sticky filter bar */}
      <div className="sticky top-0 z-10 bg-background/95 backdrop-blur-sm border-b -mx-6 px-6 py-3 space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          {/* Search */}
          <div className="relative flex-1 min-w-[180px] max-w-sm">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground" />
            <Input
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              placeholder="Search expressions..."
              className="pl-8 pr-8 h-8 text-sm"
            />
            {searchInput && (
              <button
                type="button"
                onClick={() => { setSearchInput(""); setDebouncedSearch(""); }}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              >
                <X className="size-3.5" />
              </button>
            )}
          </div>

          {/* Form select */}
          <Select
            value={formFilter || "all"}
            onValueChange={(v) => setFormFilter(v === "all" ? "" : v)}
          >
            <SelectTrigger className="w-[140px] h-8 text-sm">
              <SelectValue placeholder="All forms" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All forms</SelectItem>
              {forms.map((f) => (
                <SelectItem key={f} value={f}>
                  {formLabels[f] ?? f}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          {/* Digit range */}
          <div className="flex items-center gap-1">
            <Input
              type="number"
              min={1}
              value={minDigits}
              onChange={(e) => setMinDigits(e.target.value)}
              placeholder="Min"
              aria-invalid={parsedMinDigits === null}
              className="w-[80px] h-8 text-sm"
            />
            <span className="text-muted-foreground text-xs">-</span>
            <Input
              type="number"
              min={1}
              value={maxDigits}
              onChange={(e) => setMaxDigits(e.target.value)}
              placeholder="Max"
              aria-invalid={parsedMaxDigits === null}
              className="w-[80px] h-8 text-sm"
            />
          </div>

          {/* Sort */}
          <Select value={sortKey} onValueChange={setSortKey}>
            <SelectTrigger className="w-[140px] h-8 text-sm">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {Object.entries(SORT_OPTIONS).map(([key, label]) => (
                <SelectItem key={key} value={key}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          {/* Result count */}
          {!isInitialLoading && total > 0 && (
            <span className="text-xs text-muted-foreground ml-auto hidden sm:inline">
              {numberWithCommas(primes.primes.length)} of {numberWithCommas(total)}
            </span>
          )}
        </div>

        {/* Active filter pills */}
        {filterPills.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5">
            {filterPills.map((pill) => (
              <Badge
                key={pill.key}
                variant="secondary"
                className="text-xs gap-1 pr-1"
              >
                {pill.label}
                <button
                  type="button"
                  onClick={pill.onClear}
                  className="ml-0.5 rounded-full hover:bg-foreground/10 p-0.5"
                >
                  <X className="size-2.5" />
                </button>
              </Badge>
            ))}
            {filterPills.length > 1 && (
              <button
                type="button"
                onClick={clearFilters}
                className="text-xs text-muted-foreground hover:text-foreground transition-colors ml-1"
              >
                Clear all
              </button>
            )}
          </div>
        )}

        {digitsError && (
          <p className="text-xs text-destructive">{digitsError}</p>
        )}
      </div>

      {/* Prime list */}
      <Card className="mt-4 py-0 overflow-hidden">
        {/* Initial loading */}
        {isInitialLoading && (
          <>
            {Array.from({ length: 8 }).map((_, i) => (
              <SkeletonRow key={i} />
            ))}
          </>
        )}

        {/* Rows */}
        {!isInitialLoading && primes.primes.length > 0 && (
          <>
            {primes.primes.map((prime) => (
              <PrimeRow
                key={prime.id}
                prime={prime}
                isActive={pendingPrimeId === prime.id}
                onClick={() => handleRowClick(prime.id)}
              />
            ))}

            {/* Loading more skeletons */}
            {isLoadingMore && (
              <>
                {Array.from({ length: 3 }).map((_, i) => (
                  <SkeletonRow key={`loading-${i}`} />
                ))}
              </>
            )}

            {/* End of results */}
            {!hasMore && !isLoadingMore && (
              <div className="flex items-center justify-center py-6 text-xs text-muted-foreground">
                <span className="border-t w-8 mr-3" />
                {numberWithCommas(total)} primes
                <span className="border-t w-8 ml-3" />
              </div>
            )}
          </>
        )}

        {/* Empty state */}
        {!isInitialLoading && primes.primes.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 px-4 text-center">
            <SearchX className="size-10 text-muted-foreground/40 mb-3" />
            <p className="text-sm text-muted-foreground font-medium">
              No primes match these filters
            </p>
            {hasActiveFilters && (
              <Button
                variant="ghost"
                size="sm"
                onClick={clearFilters}
                className="mt-2"
              >
                Clear all filters
              </Button>
            )}
          </div>
        )}
      </Card>

      {/* IntersectionObserver sentinel */}
      <div ref={sentinelRef} className="h-1" />

      <PrimeDetailDialog
        prime={selectedPrime}
        open={detailOpen}
        onOpenChange={handleDetailClose}
        showVerifyButton
        loading={detailLoading}
      />
    </>
  );
}
