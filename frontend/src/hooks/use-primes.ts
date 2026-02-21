"use client";

/**
 * @module use-primes
 *
 * React hook for querying prime records via the REST API with
 * server-side filtering, sorting, and pagination. Supports filtering
 * by form, digit range, and text search on expressions.
 *
 * Exports `PrimeRecord` (list view), `PrimeDetail` (detail dialog),
 * and `PrimeFilter` (query parameters). Used by the Browse page and
 * the main dashboard primes table.
 *
 * Provides two modes:
 * - **Paginated**: `fetchPrimes(offset, limit, filter)` for the dashboard table
 * - **Infinite scroll**: `resetAndFetch(filter)` + `fetchNextPage()` for the Browse list
 */

import { useEffect, useState, useCallback, useRef } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export interface PrimeRecord {
  id: number;
  form: string;
  expression: string;
  digits: number;
  found_at: string;
  proof_method: string;
  verified: boolean;
  verified_at: string | null;
  verification_method: string | null;
  verification_tier: number | null;
}

export interface PrimeDetail {
  id: number;
  form: string;
  expression: string;
  digits: number;
  found_at: string;
  search_params: string;
  proof_method: string;
  verified: boolean;
  verified_at: string | null;
  verification_method: string | null;
  verification_tier: number | null;
}

export interface PrimeFilter {
  form?: string;
  search?: string;
  min_digits?: number;
  max_digits?: number;
  sort_by?: string;
  sort_dir?: string;
}

export interface PrimesData {
  primes: PrimeRecord[];
  total: number;
  limit: number;
  offset: number;
}

const INFINITE_PAGE_SIZE = 50;

export function usePrimes() {
  const [primes, setPrimes] = useState<PrimesData>({
    primes: [],
    total: 0,
    limit: 50,
    offset: 0,
  });
  const [selectedPrime, setSelectedPrime] = useState<PrimeDetail | null>(null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [isInitialLoading, setIsInitialLoading] = useState(false);

  // Ref to track infinite scroll state without causing re-renders
  const infiniteRef = useRef({
    filter: null as PrimeFilter | null,
    offset: 0,
    fetching: false,
  });

  const fetchPrimes = useCallback(
    async (offset: number, limit: number, filter?: PrimeFilter) => {
      const params = new URLSearchParams();
      params.set("limit", String(limit));
      params.set("offset", String(offset));
      if (filter?.form) params.set("form", filter.form);
      if (filter?.search) params.set("search", filter.search);
      if (filter?.min_digits) params.set("min_digits", String(filter.min_digits));
      if (filter?.max_digits) params.set("max_digits", String(filter.max_digits));
      if (filter?.sort_by) params.set("sort_by", filter.sort_by);
      if (filter?.sort_dir) params.set("sort_dir", filter.sort_dir);

      try {
        const res = await fetch(`${API_BASE}/api/primes?${params.toString()}`);
        if (res.ok) {
          const data = await res.json();
          setPrimes({
            primes: (data.primes as PrimeRecord[]) ?? [],
            total: data.total ?? 0,
            limit: data.limit ?? limit,
            offset: data.offset ?? offset,
          });
        }
      } catch {
        // network error — leave existing state
      }
    },
    []
  );

  /** Reset the list and fetch the first page with new filters (infinite scroll mode). */
  const resetAndFetch = useCallback(async (filter: PrimeFilter) => {
    infiniteRef.current.filter = filter;
    infiniteRef.current.offset = 0;
    infiniteRef.current.fetching = true;
    setIsInitialLoading(true);

    const params = new URLSearchParams();
    params.set("limit", String(INFINITE_PAGE_SIZE));
    params.set("offset", "0");
    if (filter.form) params.set("form", filter.form);
    if (filter.search) params.set("search", filter.search);
    if (filter.min_digits) params.set("min_digits", String(filter.min_digits));
    if (filter.max_digits) params.set("max_digits", String(filter.max_digits));
    if (filter.sort_by) params.set("sort_by", filter.sort_by);
    if (filter.sort_dir) params.set("sort_dir", filter.sort_dir);

    try {
      const res = await fetch(`${API_BASE}/api/primes?${params.toString()}`);
      if (res.ok) {
        const data = await res.json();
        const items = (data.primes as PrimeRecord[]) ?? [];
        infiniteRef.current.offset = items.length;
        setPrimes({
          primes: items,
          total: data.total ?? 0,
          limit: INFINITE_PAGE_SIZE,
          offset: 0,
        });
      }
    } catch {
      // network error — leave existing state
    } finally {
      infiniteRef.current.fetching = false;
      setIsInitialLoading(false);
    }
  }, []);

  /** Fetch the next page and append to existing results (infinite scroll mode). */
  const fetchNextPage = useCallback(async () => {
    const inf = infiniteRef.current;
    if (inf.fetching) return;
    inf.fetching = true;
    setIsLoadingMore(true);

    const filter = inf.filter ?? {};
    const params = new URLSearchParams();
    params.set("limit", String(INFINITE_PAGE_SIZE));
    params.set("offset", String(inf.offset));
    if (filter.form) params.set("form", filter.form);
    if (filter.search) params.set("search", filter.search);
    if (filter.min_digits) params.set("min_digits", String(filter.min_digits));
    if (filter.max_digits) params.set("max_digits", String(filter.max_digits));
    if (filter.sort_by) params.set("sort_by", filter.sort_by);
    if (filter.sort_dir) params.set("sort_dir", filter.sort_dir);

    try {
      const res = await fetch(`${API_BASE}/api/primes?${params.toString()}`);
      if (res.ok) {
        const data = await res.json();
        const newItems = (data.primes as PrimeRecord[]) ?? [];
        inf.offset += newItems.length;
        setPrimes((prev) => ({
          primes: [...prev.primes, ...newItems],
          total: data.total ?? prev.total,
          limit: INFINITE_PAGE_SIZE,
          offset: prev.offset,
        }));
      }
    } catch {
      // network error — leave existing state
    } finally {
      inf.fetching = false;
      setIsLoadingMore(false);
    }
  }, []);

  const hasMore = primes.primes.length < primes.total;

  const fetchPrimeDetail = useCallback(async (id: number) => {
    try {
      const res = await fetch(`${API_BASE}/api/primes/${id}`);
      if (res.ok) {
        const data = await res.json();
        setSelectedPrime(data as PrimeDetail);
      }
    } catch {
      // network error — leave existing state
    }
  }, []);

  const clearSelectedPrime = useCallback(() => {
    setSelectedPrime(null);
  }, []);

  // Initial fetch
  useEffect(() => {
    fetchPrimes(0, 50);
  }, [fetchPrimes]);

  return {
    primes,
    selectedPrime,
    fetchPrimes,
    fetchPrimeDetail,
    clearSelectedPrime,
    // Infinite scroll API
    resetAndFetch,
    fetchNextPage,
    hasMore,
    isLoadingMore,
    isInitialLoading,
  };
}
