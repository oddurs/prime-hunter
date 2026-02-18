-- Add verification columns to primes table
ALTER TABLE primes
    ADD COLUMN verified BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN verified_at TIMESTAMPTZ,
    ADD COLUMN verification_method TEXT,
    ADD COLUMN verification_tier SMALLINT;

CREATE INDEX idx_primes_unverified ON primes (id) WHERE NOT verified;
