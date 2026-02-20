-- Cost calibration: store per-form timing coefficients fitted from real data.
--
-- The cost model uses: secs_per_candidate = coeff_a * (digits/1000)^coeff_b
-- Coefficients are re-fitted periodically from completed work block data.

CREATE TABLE IF NOT EXISTS cost_calibration (
    form            TEXT PRIMARY KEY,
    coeff_a         DOUBLE PRECISION NOT NULL,
    coeff_b         DOUBLE PRECISION NOT NULL,
    sample_count    BIGINT NOT NULL DEFAULT 0,
    avg_error_pct   DOUBLE PRECISION,   -- mean absolute percentage error
    fitted_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- View: per-form timing observations from completed work blocks.
-- Each row represents one completed block with its average candidate timing.
CREATE OR REPLACE VIEW cost_observations AS
SELECT
    sj.search_type AS form,
    wb.duration_secs,
    COALESCE(wb.cores_used, 1) AS cores_used,
    (wb.range_end - wb.range_start) AS candidates,
    wb.completed_at
FROM work_blocks wb
JOIN search_jobs sj ON wb.search_job_id = sj.id
WHERE wb.status = 'completed'
  AND wb.duration_secs IS NOT NULL
  AND wb.duration_secs > 0
  AND (wb.range_end - wb.range_start) > 0;
