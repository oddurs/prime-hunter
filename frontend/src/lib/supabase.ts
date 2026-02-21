/**
 * @module supabase
 *
 * Singleton Supabase client — **auth-only** after Phase 6 migration.
 *
 * All data queries (primes, stats, schedules, projects, records, etc.)
 * have been migrated to REST endpoints served by the Rust backend.
 * This client is retained solely for Supabase Auth (login, session,
 * token refresh). See `contexts/auth-context.tsx` for usage.
 *
 * Uses `NEXT_PUBLIC_SUPABASE_URL` and `NEXT_PUBLIC_SUPABASE_ANON_KEY`
 * environment variables, with hardcoded fallbacks for the production
 * Supabase project. The anon key is safe to expose — Row Level Security
 * (RLS) policies enforce read-only public access.
 */

import { createClient } from "@supabase/supabase-js";

const supabaseUrl =
  process.env.NEXT_PUBLIC_SUPABASE_URL ||
  "https://nljvgyorzoxajodkkqdu.supabase.co";
const supabaseAnonKey =
  process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY ||
  "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Im5sanZneW9yem94YWpvZGtrcWR1Iiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzE0MTEwOTksImV4cCI6MjA4Njk4NzA5OX0.RnHwtsQjRS89_lthZ5PBXM-sL4aTkwQau0fq1xCFM3s";

export const supabase = createClient(supabaseUrl, supabaseAnonKey);
