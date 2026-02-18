-- Create primes table (migrated from SQLite)
CREATE TABLE primes (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    form TEXT NOT NULL,
    expression TEXT NOT NULL,
    digits BIGINT NOT NULL,
    found_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    search_params TEXT NOT NULL,
    proof_method TEXT NOT NULL DEFAULT 'probabilistic',
    CONSTRAINT primes_form_expression_unique UNIQUE (form, expression)
);

CREATE INDEX idx_primes_form ON primes (form);
CREATE INDEX idx_primes_digits ON primes (digits);
CREATE INDEX idx_primes_found_at ON primes (found_at);

-- Enable Supabase Realtime for live prime notifications
ALTER PUBLICATION supabase_realtime ADD TABLE primes;
