"use client";

/**
 * @module use-records
 *
 * React hook for querying world records from the `records` table in Supabase.
 *
 * The `records` table stores known world records for each prime form/category
 * (e.g., largest factorial prime, largest twin prime pair). Each record includes
 * the current holder's expression, digit count, and source — plus a comparison
 * to our best discovery for that form (`our_best_id`, `our_best_digits`).
 *
 * Records are periodically refreshed from external sources (OEIS, Prime Pages,
 * T5K) via the `POST /api/records/refresh` endpoint on the Rust backend.
 *
 * Hooks:
 * - `useRecords()` — list all world records with realtime updates
 *
 * Action functions:
 * - `refreshRecords()` — trigger a backend refresh from external sources
 *
 * @see {@link https://t5k.org} — The Prime Pages (primary source for records)
 * @see {@link https://oeis.org} — OEIS (form-specific sequence records)
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";
import { API_BASE } from "@/lib/format";

/** A world record entry for a specific prime form/category. */
export interface WorldRecord {
  id: number;
  /** Prime form (e.g., "factorial", "twin", "kbn"). */
  form: string;
  /** Record category (e.g., "largest_known", "largest_proven"). */
  category: string;
  /** Human-readable expression (e.g., "208003! - 1"). */
  expression: string;
  /** Number of decimal digits. */
  digits: number;
  /** Name or handle of the record holder. */
  holder: string | null;
  /** Date the record was set. */
  discovered_at: string | null;
  /** Source name (e.g., "T5K", "OEIS", "PrimeGrid"). */
  source: string | null;
  /** URL to the source page for this record. */
  source_url: string | null;
  /** ID of our best prime in this form (FK to primes.id), or null. */
  our_best_id: number | null;
  /** Digit count of our best prime in this form. */
  our_best_digits: number | null;
  /** When the record was last fetched from the external source. */
  fetched_at: string | null;
  /** When this row was last updated in our database. */
  updated_at: string;
}

/**
 * Fetch all world records from the `records` table with realtime updates.
 *
 * Returns records ordered by form and category. Subscribes to Supabase
 * Realtime for INSERT/UPDATE/DELETE on the table so the UI stays current
 * after a refresh.
 */
export function useRecords() {
  const [records, setRecords] = useState<WorldRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchRecords = useCallback(async () => {
    const { data, error: queryError } = await supabase
      .from("records")
      .select("*")
      .order("form")
      .order("category");

    if (queryError) {
      setError(queryError.message);
    } else if (data) {
      setRecords(data as WorldRecord[]);
      setError(null);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchRecords();
  }, [fetchRecords]);

  // Realtime subscription for record changes
  useEffect(() => {
    const channel = supabase
      .channel("records_changes")
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "records" },
        () => {
          fetchRecords();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchRecords]);

  return { records, loading, error, refetch: fetchRecords };
}

/**
 * Trigger a refresh of world records from external sources.
 *
 * Sends `POST /api/records/refresh` to the Rust backend, which fetches
 * the latest records from T5K, OEIS, PrimeGrid, etc. and upserts them
 * into the `records` table.
 */
export async function refreshRecords(): Promise<void> {
  const resp = await fetch(`${API_BASE}/api/records/refresh`, {
    method: "POST",
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error((body as Record<string, string>).error || "Failed to refresh records");
  }
}
