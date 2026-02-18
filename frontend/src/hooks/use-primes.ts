"use client";

/**
 * @module use-primes
 *
 * React hook for querying the `primes` table via Supabase with
 * server-side filtering, sorting, and pagination. Supports filtering
 * by form, digit range, proof method, and text search on expressions.
 *
 * Exports `PrimeRecord` (list view), `PrimeDetail` (detail dialog),
 * and `PrimeFilter` (query parameters). Used by the Browse page and
 * the main dashboard primes table.
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";

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

export function usePrimes() {
  const [primes, setPrimes] = useState<PrimesData>({
    primes: [],
    total: 0,
    limit: 50,
    offset: 0,
  });
  const [selectedPrime, setSelectedPrime] = useState<PrimeDetail | null>(null);

  const fetchPrimes = useCallback(
    async (offset: number, limit: number, filter?: PrimeFilter) => {
      let query = supabase
        .from("primes")
        .select("id, form, expression, digits, found_at, proof_method, verified, verified_at, verification_method, verification_tier", {
          count: "exact",
        });

      if (filter?.form) {
        query = query.eq("form", filter.form);
      }
      if (filter?.search) {
        query = query.ilike("expression", `%${filter.search}%`);
      }
      if (filter?.min_digits) {
        query = query.gte("digits", filter.min_digits);
      }
      if (filter?.max_digits) {
        query = query.lte("digits", filter.max_digits);
      }

      const sortCol = filter?.sort_by || "id";
      const ascending = filter?.sort_dir === "asc";
      query = query.order(sortCol, { ascending });

      query = query.range(offset, offset + limit - 1);

      const { data, count, error } = await query;
      if (!error) {
        setPrimes({
          primes: (data as PrimeRecord[]) ?? [],
          total: count ?? 0,
          limit,
          offset,
        });
      }
    },
    []
  );

  const fetchPrimeDetail = useCallback(async (id: number) => {
    const { data, error } = await supabase
      .from("primes")
      .select("id, form, expression, digits, found_at, search_params, proof_method, verified, verified_at, verification_method, verification_tier")
      .eq("id", id)
      .single();
    if (!error && data) {
      setSelectedPrime(data as PrimeDetail);
    }
  }, []);

  const clearSelectedPrime = useCallback(() => {
    setSelectedPrime(null);
  }, []);

  // Initial fetch
  useEffect(() => {
    fetchPrimes(0, 50);
  }, [fetchPrimes]);

  return { primes, selectedPrime, fetchPrimes, fetchPrimeDetail, clearSelectedPrime };
}
