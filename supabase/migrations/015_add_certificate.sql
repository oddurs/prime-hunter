-- Add exportable primality certificate column.
-- Stores witness data (Proth base, LLR seed, Pocklington/Morrison witnesses, etc.)
-- as JSONB for independent verification without re-running expensive tests.
ALTER TABLE primes ADD COLUMN IF NOT EXISTS certificate JSONB;
