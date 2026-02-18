-- Projects: campaign-style prime discovery management.
-- A project groups one or more phased search jobs together with a goal
-- (record-hunting, survey, verification, or custom research), budget
-- tracking, and competitive record comparison.

-- ── projects ──────────────────────────────────────────────────────
CREATE TABLE projects (
    id              BIGSERIAL PRIMARY KEY,
    slug            TEXT UNIQUE NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    objective       TEXT NOT NULL CHECK (objective IN ('record','survey','verification','custom')),
    form            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft','active','paused','completed','cancelled','failed')),
    toml_source     TEXT,
    target          JSONB NOT NULL DEFAULT '{}',
    competitive     JSONB NOT NULL DEFAULT '{}',
    strategy        JSONB NOT NULL DEFAULT '{}',
    infrastructure  JSONB NOT NULL DEFAULT '{}',
    budget          JSONB NOT NULL DEFAULT '{}',
    total_tested    BIGINT NOT NULL DEFAULT 0,
    total_found     BIGINT NOT NULL DEFAULT 0,
    best_prime_id   BIGINT REFERENCES primes(id),
    best_digits     BIGINT NOT NULL DEFAULT 0,
    total_core_hours    DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_cost_usd      DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── project_phases ───────────────────────────────────────────────
CREATE TABLE project_phases (
    id              BIGSERIAL PRIMARY KEY,
    project_id      BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    phase_order     INT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending','active','completed','skipped','failed')),
    search_params   JSONB NOT NULL,
    block_size      BIGINT NOT NULL DEFAULT 1000,
    depends_on      TEXT[] NOT NULL DEFAULT '{}',
    activation_condition TEXT,
    completion_condition TEXT NOT NULL DEFAULT 'all_blocks_done',
    search_job_id   BIGINT REFERENCES search_jobs(id),
    total_tested    BIGINT NOT NULL DEFAULT 0,
    total_found     BIGINT NOT NULL DEFAULT 0,
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    UNIQUE (project_id, name)
);

-- ── records ──────────────────────────────────────────────────────
-- World records per prime form, compared against our discoveries.
CREATE TABLE records (
    id              BIGSERIAL PRIMARY KEY,
    form            TEXT NOT NULL,
    category        TEXT NOT NULL DEFAULT 'overall',
    expression      TEXT NOT NULL,
    digits          BIGINT NOT NULL,
    holder          TEXT,
    discovered_at   DATE,
    source          TEXT,
    source_url      TEXT,
    our_best_id     BIGINT REFERENCES primes(id),
    our_best_digits BIGINT NOT NULL DEFAULT 0,
    fetched_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (form, category)
);

-- ── project_events ───────────────────────────────────────────────
CREATE TABLE project_events (
    id              BIGSERIAL PRIMARY KEY,
    project_id      BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_type      TEXT NOT NULL,
    summary         TEXT NOT NULL,
    detail          JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── FK: link search_jobs to projects ─────────────────────────────
ALTER TABLE search_jobs ADD COLUMN IF NOT EXISTS project_id BIGINT REFERENCES projects(id);
