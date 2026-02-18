-- Enable Row Level Security on primes table
ALTER TABLE primes ENABLE ROW LEVEL SECURITY;

-- All authenticated users can read (single-tenant: everyone sees everything)
CREATE POLICY "Authenticated users can read primes"
    ON primes
    FOR SELECT
    TO authenticated
    USING (true);

-- Service role (used by Rust coordinator) bypasses RLS by default,
-- so no explicit INSERT policy is needed for the backend.
