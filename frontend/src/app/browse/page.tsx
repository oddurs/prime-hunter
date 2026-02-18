"use client";

/**
 * @module browse/page
 *
 * Primes browser page with a full-featured data table. Supports:
 *
 * - **Filtering**: by form, digit range, proof method, text search
 * - **Sorting**: by any column (expression, digits, date, form)
 * - **Pagination**: server-side via Supabase `.range()` queries
 * - **Column visibility**: toggle columns on/off, resizable widths
 * - **Detail dialog**: click a row to see full prime details + verify
 *
 * Uses `@tanstack/react-table` for headless table logic and the
 * `usePrimes()` hook for Supabase-backed data fetching.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import {
  ColumnDef,
  ColumnSizingState,
  SortingState,
  VisibilityState,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { CheckCircle2, Clock, ExternalLink, RefreshCw, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { usePrimes, type PrimeFilter, type PrimeRecord } from "@/hooks/use-primes";
import { useStats } from "@/hooks/use-stats";
import { API_BASE, formToSlug, formatTime, numberWithCommas } from "@/lib/format";
import { cn } from "@/lib/utils";
import { ViewHeader } from "@/components/view-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type Density = "compact" | "comfortable";

const defaultColumnVisibility: VisibilityState = {
  id: false,
};

function parsePositiveInteger(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return 0;
  }
  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 1) {
    return null;
  }
  return parsed;
}

export default function BrowsePage() {
  const { stats } = useStats();
  const {
    primes,
    selectedPrime,
    fetchPrimes,
    fetchPrimeDetail,
    clearSelectedPrime,
  } = usePrimes();

  const [searchInput, setSearchInput] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [formFilter, setFormFilter] = useState<string>("");
  const [minDigits, setMinDigits] = useState("");
  const [maxDigits, setMaxDigits] = useState("");
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>(
    defaultColumnVisibility
  );
  const [columnSizing, setColumnSizing] = useState<ColumnSizingState>({});
  const [density, setDensity] = useState<Density>("comfortable");
  const [wrapExpressions, setWrapExpressions] = useState(false);
  const [pageSize, setPageSize] = useState(primes.limit);
  const [detailOpen, setDetailOpen] = useState(false);
  const [pendingPrimeId, setPendingPrimeId] = useState<number | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [verifying, setVerifying] = useState(false);

  const offset = primes.offset;
  const total = primes.total;

  const forms = useMemo(() => {
    const fromStats = stats?.by_form?.map((f) => f.form) ?? [];
    const fromPrimes = primes.primes.map((p) => p.form);
    return Array.from(new Set([...fromStats, ...fromPrimes])).sort();
  }, [stats?.by_form, primes.primes]);

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(searchInput);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchInput]);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const query = params.get("q");
    const form = params.get("form");
    const min = params.get("min_digits");
    const max = params.get("max_digits");
    const rows = params.get("rows");
    const sortBy = params.get("sort_by");
    const sortDir = params.get("sort_dir");
    const prime = params.get("prime");

    if (query) {
      setSearchInput(query);
      setDebouncedSearch(query);
    }
    if (form) {
      setFormFilter(form);
    }
    if (min) {
      setMinDigits(min);
    }
    if (max) {
      setMaxDigits(max);
    }
    if (rows) {
      const parsedRows = Number(rows);
      if ([25, 50, 100].includes(parsedRows)) {
        setPageSize(parsedRows);
      }
    }
    if (sortBy) {
      setSorting([{ id: sortBy, desc: sortDir === "desc" }]);
    }
    if (prime) {
      const parsedPrime = Number(prime);
      if (Number.isInteger(parsedPrime) && parsedPrime > 0) {
        setPendingPrimeId(parsedPrime);
        setDetailOpen(true);
      }
    }
  }, []);

  useEffect(() => {
    const params = new URLSearchParams();
    if (debouncedSearch) params.set("q", debouncedSearch);
    if (formFilter) params.set("form", formFilter);
    if (minDigits.trim()) params.set("min_digits", minDigits.trim());
    if (maxDigits.trim()) params.set("max_digits", maxDigits.trim());
    if (sorting.length > 0) {
      params.set("sort_by", sorting[0].id);
      params.set("sort_dir", sorting[0].desc ? "desc" : "asc");
    }
    if (pageSize !== 50) params.set("rows", String(pageSize));
    if (detailOpen && pendingPrimeId !== null) {
      params.set("prime", String(pendingPrimeId));
    }

    const query = params.toString();
    const targetUrl = query ? `/browse?${query}` : "/browse";
    window.history.replaceState({}, "", targetUrl);
  }, [debouncedSearch, formFilter, minDigits, maxDigits, sorting, pageSize, detailOpen, pendingPrimeId]);

  const parsedMinDigits = useMemo(() => parsePositiveInteger(minDigits), [minDigits]);
  const parsedMaxDigits = useMemo(() => parsePositiveInteger(maxDigits), [maxDigits]);

  const digitsError = useMemo(() => {
    if (parsedMinDigits === null || parsedMaxDigits === null) {
      return "Digit filters must be positive integers.";
    }
    if (
      parsedMinDigits > 0 &&
      parsedMaxDigits > 0 &&
      parsedMinDigits > parsedMaxDigits
    ) {
      return "Min digits cannot be greater than max digits.";
    }
    return null;
  }, [parsedMinDigits, parsedMaxDigits]);

  const buildFilter = useCallback((): PrimeFilter => {
    const f: PrimeFilter = {};
    if (formFilter) f.form = formFilter;
    if (debouncedSearch) f.search = debouncedSearch;
    if (parsedMinDigits && parsedMinDigits > 0) {
      f.min_digits = parsedMinDigits;
    }
    if (parsedMaxDigits && parsedMaxDigits > 0) {
      f.max_digits = parsedMaxDigits;
    }
    if (sorting.length > 0) {
      const active = sorting[0];
      f.sort_by = active.id;
      f.sort_dir = active.desc ? "desc" : "asc";
    }
    return f;
  }, [formFilter, debouncedSearch, parsedMinDigits, parsedMaxDigits, sorting]);

  useEffect(() => {
    if (digitsError) {
      return;
    }
    fetchPrimes(0, pageSize, buildFilter());
  }, [debouncedSearch, formFilter, minDigits, maxDigits, sorting, pageSize, digitsError, buildFilter, fetchPrimes]);

  useEffect(() => {
    if (pendingPrimeId === null || !detailOpen) return;
    clearSelectedPrime();
    setDetailLoading(true);
    fetchPrimeDetail(pendingPrimeId);
  }, [pendingPrimeId, detailOpen, fetchPrimeDetail, clearSelectedPrime]);

  useEffect(() => {
    if (!selectedPrime || pendingPrimeId === null) return;
    if (selectedPrime.id === pendingPrimeId) {
      setDetailLoading(false);
    }
  }, [selectedPrime, pendingPrimeId]);

  const hasActiveFilters = !!(
    formFilter ||
    debouncedSearch ||
    minDigits ||
    maxDigits ||
    sorting.length > 0
  );

  function clearFilters() {
    setSearchInput("");
    setDebouncedSearch("");
    setFormFilter("");
    setMinDigits("");
    setMaxDigits("");
    setSorting([]);
  }

  function prevPage() {
    fetchPrimes(Math.max(0, offset - pageSize), pageSize, buildFilter());
  }

  function nextPage() {
    if (offset + pageSize < total) {
      fetchPrimes(offset + pageSize, pageSize, buildFilter());
    }
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

  const handleVerify = useCallback(async () => {
    if (!selectedPrime) return;
    setVerifying(true);
    try {
      const res = await fetch(
        `${API_BASE}/api/primes/${selectedPrime.id}/verify`,
        { method: "POST" }
      );
      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      if (data.result === "verified") {
        toast.success(`Verified: ${data.method} (Tier ${data.tier})`);
      } else if (data.result === "failed") {
        toast.error(`Verification failed: ${data.reason}`);
      } else if (data.result === "skipped") {
        toast.info(`Verification skipped: ${data.reason}`);
      }
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Verification request failed";
      toast.error(message);
    } finally {
      setVerifying(false);
    }
  }, [selectedPrime]);

  function exportData(format: "csv" | "json") {
    if (digitsError) return;
    const params = new URLSearchParams();
    params.set("format", format);
    if (formFilter) params.set("form", formFilter);
    if (debouncedSearch) params.set("search", debouncedSearch);
    if (parsedMinDigits && parsedMinDigits > 0) {
      params.set("min_digits", String(parsedMinDigits));
    }
    if (parsedMaxDigits && parsedMaxDigits > 0) {
      params.set("max_digits", String(parsedMaxDigits));
    }
    if (sorting.length > 0) {
      params.set("sort_by", sorting[0].id);
      params.set("sort_dir", sorting[0].desc ? "desc" : "asc");
    }
    window.open(`${API_BASE}/api/export?${params.toString()}`, "_blank");
  }

  const columns = useMemo<ColumnDef<PrimeRecord>[]>(() => {
    return [
      {
        id: "expression",
        header: "Expression",
        accessorKey: "expression",
        enableHiding: false,
        size: 360,
        cell: ({ row }) => (
          <span
            className={cn(
              "font-mono text-primary inline-flex items-center gap-1.5",
              wrapExpressions ? "whitespace-normal" : "whitespace-nowrap"
            )}
          >
            {row.original.verified ? (
              <CheckCircle2 className="size-3.5 text-green-500 shrink-0" />
            ) : (
              <Clock className="size-3.5 text-muted-foreground/50 shrink-0" />
            )}
            {row.original.expression}
          </span>
        ),
      },
      {
        id: "form",
        header: "Form",
        accessorKey: "form",
        size: 140,
        cell: ({ row }) => (
          <Link
            href={`/docs?doc=${formToSlug(row.original.form)}`}
            onClick={(event) => event.stopPropagation()}
          >
            <Badge
              variant="outline"
              className="cursor-pointer hover:bg-secondary/50"
            >
              {row.original.form}
            </Badge>
          </Link>
        ),
      },
      {
        id: "digits",
        header: "Digits",
        accessorKey: "digits",
        size: 120,
        cell: ({ row }) => numberWithCommas(row.original.digits),
      },
      {
        id: "found_at",
        header: "Found",
        accessorKey: "found_at",
        size: 180,
        cell: ({ row }) => (
          <span className="text-muted-foreground">
            {formatTime(row.original.found_at)}
          </span>
        ),
      },
      {
        id: "id",
        header: "ID",
        accessorKey: "id",
        size: 80,
      },
    ];
  }, [wrapExpressions]);

  const table = useReactTable({
    data: primes.primes,
    columns,
    state: {
      sorting,
      columnVisibility,
      columnSizing,
    },
    manualSorting: true,
    manualFiltering: true,
    enableColumnResizing: true,
    columnResizeMode: "onChange",
    onSortingChange: setSorting,
    onColumnVisibilityChange: setColumnVisibility,
    onColumnSizingChange: setColumnSizing,
    getCoreRowModel: getCoreRowModel(),
  });

  const rowPadding = density === "compact" ? "py-1" : "py-2";

  function resetView() {
    setColumnVisibility(defaultColumnVisibility);
    setColumnSizing({});
    setDensity("comfortable");
    setWrapExpressions(false);
  }

  let parsedSearchParams: Record<string, unknown> | null = null;
  if (selectedPrime?.search_params) {
    try {
      parsedSearchParams = JSON.parse(selectedPrime.search_params);
    } catch {
      // leave as null
    }
  }

  return (
    <>
      <ViewHeader
        title="Browse"
        subtitle={
          total === 0
            ? "No primes yet"
            : `${numberWithCommas(total)} primes in the archive`
        }
        actions={
          <>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="outline" size="sm">
                  View
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-56">
                <DropdownMenuLabel>Columns</DropdownMenuLabel>
                {table.getAllLeafColumns().map((column) => (
                  <DropdownMenuCheckboxItem
                    key={column.id}
                    checked={column.getIsVisible()}
                    onCheckedChange={(checked) =>
                      column.toggleVisibility(!!checked)
                    }
                    disabled={!column.getCanHide()}
                  >
                    {column.columnDef.header as string}
                  </DropdownMenuCheckboxItem>
                ))}
                <DropdownMenuSeparator />
                <DropdownMenuLabel>Density</DropdownMenuLabel>
                <DropdownMenuRadioGroup
                  value={density}
                  onValueChange={(value) => setDensity(value as Density)}
                >
                  <DropdownMenuRadioItem value="comfortable">
                    Comfortable
                  </DropdownMenuRadioItem>
                  <DropdownMenuRadioItem value="compact">
                    Compact
                  </DropdownMenuRadioItem>
                </DropdownMenuRadioGroup>
                <DropdownMenuSeparator />
                <DropdownMenuCheckboxItem
                  checked={wrapExpressions}
                  onCheckedChange={(checked) => setWrapExpressions(!!checked)}
                >
                  Wrap expressions
                </DropdownMenuCheckboxItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem onClick={resetView}>Reset view</DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>

            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="outline" size="sm">
                  Export
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  onClick={() => exportData("csv")}
                  disabled={!!digitsError}
                >
                  Export CSV
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => exportData("json")}
                  disabled={!!digitsError}
                >
                  Export JSON
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </>
        }
        className="mb-6"
      />

      <Card className="mb-4">
        <CardContent className="p-4">
          <div className="flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-[minmax(220px,1fr)_200px_140px_140px]">
              <div className="space-y-1">
                <label className="text-xs font-medium text-muted-foreground">
                  Search
                </label>
                <Input
                  value={searchInput}
                  onChange={(event) => setSearchInput(event.target.value)}
                  placeholder="Expression contains..."
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium text-muted-foreground">
                  Form
                </label>
                <Select
                  value={formFilter || "all"}
                  onValueChange={(value) =>
                    setFormFilter(value === "all" ? "" : value)
                  }
                >
                  <SelectTrigger>
                    <SelectValue placeholder="All forms" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">All forms</SelectItem>
                    {forms.map((form) => (
                      <SelectItem key={form} value={form}>
                        {form}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium text-muted-foreground">
                  Min digits
                </label>
                <Input
                  type="number"
                  min={1}
                  aria-invalid={parsedMinDigits === null}
                  value={minDigits}
                  onChange={(event) => setMinDigits(event.target.value)}
                  placeholder="e.g. 100"
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium text-muted-foreground">
                  Max digits
                </label>
                <Input
                  type="number"
                  min={1}
                  aria-invalid={parsedMaxDigits === null}
                  value={maxDigits}
                  onChange={(event) => setMaxDigits(event.target.value)}
                  placeholder="e.g. 2000"
                />
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Select
                value={String(pageSize)}
                onValueChange={(value) => setPageSize(Number(value))}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Rows" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="25">25 rows</SelectItem>
                  <SelectItem value="50">50 rows</SelectItem>
                  <SelectItem value="100">100 rows</SelectItem>
                </SelectContent>
              </Select>
              {hasActiveFilters && (
                <Button variant="ghost" size="sm" onClick={clearFilters}>
                  Clear filters
                </Button>
              )}
            </div>
          </div>
          {digitsError && (
            <p className="mt-3 text-xs text-destructive">{digitsError}</p>
          )}
        </CardContent>
      </Card>

      <Card className="py-0 overflow-hidden">
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  const isSorted = header.column.getIsSorted();
                  return (
                    <TableHead
                      key={header.id}
                      style={{ width: header.getSize() }}
                      className={cn(
                        "relative text-xs font-medium text-muted-foreground",
                        header.column.getCanSort() && "cursor-pointer select-none"
                      )}
                      onClick={header.column.getToggleSortingHandler()}
                    >
                      <div className="flex items-center gap-2">
                        {header.isPlaceholder
                          ? null
                          : flexRender(
                              header.column.columnDef.header,
                              header.getContext()
                            )}
                        {isSorted && (
                          <span className="text-xs text-muted-foreground">
                            {isSorted === "asc" ? "↑" : "↓"}
                          </span>
                        )}
                      </div>
                      {header.column.getCanResize() && (
                        <div
                          onMouseDown={header.getResizeHandler()}
                          onTouchStart={header.getResizeHandler()}
                          className={cn(
                            "absolute right-0 top-0 h-full w-1 cursor-col-resize select-none touch-none",
                            header.column.getIsResizing()
                              ? "bg-primary/40"
                              : "bg-transparent hover:bg-border"
                          )}
                        />
                      )}
                    </TableHead>
                  );
                })}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {table.getRowModel().rows.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={table.getVisibleLeafColumns().length}
                  className="text-center text-muted-foreground py-8"
                >
                  No primes match these filters
                </TableCell>
              </TableRow>
            ) : (
              table.getRowModel().rows.map((row) => (
                <TableRow
                  key={row.id}
                  className={cn(
                    "cursor-pointer",
                    pendingPrimeId === row.original.id && "bg-muted/40"
                  )}
                  role="button"
                  tabIndex={0}
                  onClick={() => handleRowClick(row.original.id)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      handleRowClick(row.original.id);
                    }
                  }}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell
                      key={cell.id}
                      style={{ width: cell.column.getSize() }}
                      className={cn(rowPadding)}
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </Card>

      <div className="flex flex-col gap-3 mt-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="text-sm text-muted-foreground">
          {total === 0
            ? "0 results"
            : `${offset + 1}-${Math.min(offset + pageSize, total)} of ${numberWithCommas(total)}`}
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={prevPage}
            disabled={offset === 0}
          >
            Previous
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={nextPage}
            disabled={offset + pageSize >= total}
          >
            Next
          </Button>
        </div>
      </div>

      <Dialog open={detailOpen} onOpenChange={handleDetailClose}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle className="font-mono text-primary break-all">
              {detailLoading ? "Loading..." : (selectedPrime?.expression ?? "Prime detail")}
            </DialogTitle>
          </DialogHeader>
          {detailLoading && (
            <p className="text-sm text-muted-foreground">Loading prime details...</p>
          )}
          {!detailLoading && selectedPrime && (
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Form
                  </div>
                  <Badge variant="outline">{selectedPrime.form}</Badge>
                </div>
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Digits
                  </div>
                  <span className="font-semibold">
                    {numberWithCommas(selectedPrime.digits)}
                  </span>
                </div>
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Proof
                  </div>
                  <Badge variant="outline">{selectedPrime.proof_method}</Badge>
                </div>
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Verification
                  </div>
                  {selectedPrime.verified ? (
                    <span className="inline-flex items-center gap-1 text-green-600 dark:text-green-400 font-medium">
                      <CheckCircle2 className="size-3.5" />
                      Tier {selectedPrime.verification_tier}
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 text-muted-foreground">
                      <Clock className="size-3.5" />
                      Pending
                    </span>
                  )}
                </div>
                <div className="col-span-2">
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Found at
                  </div>
                  <span>{formatTime(selectedPrime.found_at)}</span>
                </div>
              </div>
              {selectedPrime.verified && selectedPrime.verification_method && (
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Verification details
                  </div>
                  <div className="bg-muted rounded-md p-3 text-xs space-y-1">
                    <div><span className="text-muted-foreground">Method:</span> {selectedPrime.verification_method}</div>
                    {selectedPrime.verified_at && (
                      <div><span className="text-muted-foreground">Verified at:</span> {formatTime(selectedPrime.verified_at)}</div>
                    )}
                  </div>
                </div>
              )}
              {parsedSearchParams && (
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Search parameters
                  </div>
                  <pre className="bg-muted rounded-md p-3 text-xs overflow-auto max-h-48">
                    {JSON.stringify(parsedSearchParams, null, 2)}
                  </pre>
                </div>
              )}
              {selectedPrime.search_params && !parsedSearchParams && (
                <div>
                  <div className="text-xs font-medium text-muted-foreground mb-1">
                    Search parameters
                  </div>
                  <pre className="bg-muted rounded-md p-3 text-xs overflow-auto max-h-48">
                    {selectedPrime.search_params}
                  </pre>
                </div>
              )}
              <div className="flex items-center gap-2 pt-2 border-t">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleVerify}
                  disabled={verifying}
                >
                  {verifying ? (
                    <Loader2 className="size-3.5 mr-1 animate-spin" />
                  ) : (
                    <RefreshCw className="size-3.5 mr-1" />
                  )}
                  {verifying ? "Verifying..." : "Re-verify"}
                </Button>
                <Button variant="outline" size="sm" asChild>
                  <Link href={`/prime/?id=${selectedPrime.id}`}>
                    <ExternalLink className="size-3.5 mr-1" />
                    Permalink
                  </Link>
                </Button>
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}
