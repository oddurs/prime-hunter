/**
 * @module primes-table
 *
 * Self-contained primes archive table with search, form filtering,
 * sortable columns, pagination, CSV/JSON export, and prime detail
 * dialog. Owns its own `usePrimes` hook instance for independent
 * data fetching and filter state.
 *
 * Used on the main dashboard in the "Prime Archive" section.
 */

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { Card, CardContent } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { usePrimes, type PrimeFilter } from "@/hooks/use-primes";
import { PrimeDetailDialog } from "@/components/prime-detail-dialog";
import { API_BASE, numberWithCommas, formToSlug } from "@/lib/format";
import type { Stats } from "@/hooks/use-stats";

type SortColumn = "expression" | "form" | "digits" | "found_at" | undefined;
type SortDir = "asc" | "desc" | undefined;

interface PrimesTableProps {
  /** Stats for the form dropdown filter. */
  stats: Stats | null;
}

export function PrimesTable({ stats }: PrimesTableProps) {
  const { primes, selectedPrime, fetchPrimes, fetchPrimeDetail, clearSelectedPrime } = usePrimes();

  const [searchInput, setSearchInput] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [formFilter, setFormFilter] = useState<string>("");
  const [sortBy, setSortBy] = useState<SortColumn>(undefined);
  const [sortDir, setSortDir] = useState<SortDir>(undefined);
  const [detailOpen, setDetailOpen] = useState(false);

  const offset = primes.offset;
  const limit = primes.limit;
  const total = primes.total;

  // Debounce search input
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(searchInput);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchInput]);

  // Build current filter
  const buildFilter = useCallback((): PrimeFilter => {
    const f: PrimeFilter = {};
    if (formFilter) f.form = formFilter;
    if (debouncedSearch) f.search = debouncedSearch;
    if (sortBy) {
      f.sort_by = sortBy;
      f.sort_dir = sortDir;
    }
    return f;
  }, [formFilter, debouncedSearch, sortBy, sortDir]);

  // Re-fetch when filters change
  useEffect(() => {
    fetchPrimes(0, limit, buildFilter());
  }, [debouncedSearch, formFilter, sortBy, sortDir, limit, fetchPrimes, buildFilter]);

  const hasActiveFilters = !!(formFilter || debouncedSearch || sortBy);

  function clearFilters() {
    setSearchInput("");
    setDebouncedSearch("");
    setFormFilter("");
    setSortBy(undefined);
    setSortDir(undefined);
  }

  function handleSort(column: SortColumn) {
    if (sortBy === column) {
      if (sortDir === "asc") {
        setSortDir("desc");
      } else {
        setSortBy(undefined);
        setSortDir(undefined);
      }
    } else {
      setSortBy(column);
      setSortDir("asc");
    }
  }

  function sortIndicator(column: SortColumn) {
    if (sortBy !== column) return " \u2195";
    return sortDir === "asc" ? " \u2191" : " \u2193";
  }

  function prevPage() {
    fetchPrimes(Math.max(0, offset - limit), limit, buildFilter());
  }

  function nextPage() {
    if (offset + limit < total) {
      fetchPrimes(offset + limit, limit, buildFilter());
    }
  }

  function handleRowClick(id: number) {
    fetchPrimeDetail(id);
    setDetailOpen(true);
  }

  function handleDetailClose(open: boolean) {
    if (!open) {
      setDetailOpen(false);
      clearSelectedPrime();
    }
  }

  function exportData(format: "csv" | "json") {
    const params = new URLSearchParams();
    params.set("format", format);
    if (formFilter) params.set("form", formFilter);
    if (debouncedSearch) params.set("search", debouncedSearch);
    if (sortBy) {
      params.set("sort_by", sortBy);
      if (sortDir) params.set("sort_dir", sortDir);
    }
    window.open(`${API_BASE}/api/export?${params.toString()}`, "_blank");
  }

  const formatTime = (iso: string) => new Date(iso).toLocaleString();

  return (
    <>
      <Card id="primes-section" className="mb-4 scroll-mt-6 border">
        <CardContent className="p-4">
          <div className="flex flex-wrap items-center justify-between gap-3 mb-3">
            <h2 className="text-base font-semibold text-foreground">
              {hasActiveFilters ? "Filtered primes" : "Recent primes"}
            </h2>
            <span className="text-sm text-muted-foreground">
              {total === 0
                ? "0 results"
                : `${offset + 1}-${Math.min(offset + limit, total)} of ${numberWithCommas(total)}`}
            </span>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <div className="relative flex-1 min-w-[200px] max-w-[300px]">
              <svg
                className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.3-4.3" />
              </svg>
              <Input
                placeholder="Search expressions..."
                value={searchInput}
                onChange={(e) => setSearchInput(e.target.value)}
                className="pl-9 pr-8"
              />
              {searchInput && (
                <button
                  onClick={() => setSearchInput("")}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  aria-label="Clear search"
                >
                  <svg
                    className="h-4 w-4"
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M18 6 6 18" />
                    <path d="m6 6 12 12" />
                  </svg>
                </button>
              )}
            </div>

            <Select
              value={formFilter || "all"}
              onValueChange={(v) => setFormFilter(v === "all" ? "" : v)}
            >
              <SelectTrigger className="w-[160px]">
                <SelectValue placeholder="All forms" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All forms</SelectItem>
                {stats?.by_form.map((f) => (
                  <SelectItem key={f.form} value={f.form}>
                    {f.form} ({f.count})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="outline" size="sm">
                  <svg
                    className="h-4 w-4 mr-1.5"
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                    <polyline points="7 10 12 15 17 10" />
                    <line x1="12" x2="12" y1="15" y2="3" />
                  </svg>
                  Export
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent>
                <DropdownMenuItem onClick={() => exportData("csv")}>
                  Export CSV
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => exportData("json")}>
                  Export JSON
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>

            {hasActiveFilters && (
              <Button variant="ghost" size="sm" onClick={clearFilters}>
                Clear filters
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Primes table */}
      <Card className="py-0 overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead
                className="text-xs font-medium text-muted-foreground cursor-pointer select-none hover:text-foreground"
                onClick={() => handleSort("expression")}
              >
                Expression{sortIndicator("expression")}
              </TableHead>
              <TableHead
                className="text-xs font-medium text-muted-foreground cursor-pointer select-none hover:text-foreground"
                onClick={() => handleSort("form")}
              >
                Form{sortIndicator("form")}
              </TableHead>
              <TableHead
                className="text-xs font-medium text-muted-foreground cursor-pointer select-none hover:text-foreground"
                onClick={() => handleSort("digits")}
              >
                Digits{sortIndicator("digits")}
              </TableHead>
              <TableHead
                className="text-xs font-medium text-muted-foreground cursor-pointer select-none hover:text-foreground"
                onClick={() => handleSort("found_at")}
              >
                Found{sortIndicator("found_at")}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {primes.primes.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={4}
                  className="text-center text-muted-foreground py-8"
                >
                  No primes found yet
                </TableCell>
              </TableRow>
            ) : (
              primes.primes.map((p) => (
                <TableRow
                  key={p.id}
                  className="cursor-pointer"
                  onClick={() => handleRowClick(p.id)}
                >
                  <TableCell className="font-mono text-primary">
                    {p.expression}
                  </TableCell>
                  <TableCell>
                    <Link
                      href={`/docs?doc=${formToSlug(p.form)}`}
                      onClick={(e) => e.stopPropagation()}
                    >
                      <Badge
                        variant="outline"
                        className="cursor-pointer hover:bg-secondary/50"
                      >
                        {p.form}
                      </Badge>
                    </Link>
                  </TableCell>
                  <TableCell>{numberWithCommas(p.digits)}</TableCell>
                  <TableCell className="text-muted-foreground">
                    {formatTime(p.found_at)}
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </Card>

      {/* Pagination */}
      <div className="flex items-center justify-between gap-4 mt-4">
        <Button
          variant="outline"
          size="sm"
          onClick={prevPage}
          disabled={offset === 0}
        >
          Previous
        </Button>
        <span className="text-sm text-muted-foreground">
          {total === 0
            ? "0"
            : `${offset + 1}-${Math.min(offset + limit, total)}`}{" "}
          of {numberWithCommas(total)}
        </span>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={nextPage}
            disabled={offset + limit >= total}
          >
            Next
          </Button>
        </div>
      </div>

      <PrimeDetailDialog
        prime={selectedPrime}
        open={detailOpen}
        onOpenChange={handleDetailClose}
      />
    </>
  );
}
