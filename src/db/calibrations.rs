//! Cost calibration operations.
//!
//! The cost model estimates computation time per candidate using a power-law:
//!
//! ```text
//! secs_per_candidate = coeff_a * (digits / 1000) ^ coeff_b
//! ```
//!
//! Coefficients are fitted periodically from completed work block data stored
//! in the `cost_calibration` table (one row per form). The `cost_observations`
//! view aggregates timing data from `work_blocks` for the fitting process.

use super::{CostCalibrationRow, Database};
use anyhow::Result;

impl Database {
    /// Get all cost calibration coefficients, one row per form.
    pub async fn get_cost_calibrations(&self) -> Result<Vec<CostCalibrationRow>> {
        let rows = sqlx::query_as::<_, CostCalibrationRow>(
            "SELECT form, coeff_a, coeff_b, sample_count, avg_error_pct, fitted_at
             FROM cost_calibration ORDER BY form",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get calibration for a specific form.
    pub async fn get_cost_calibration(&self, form: &str) -> Result<Option<CostCalibrationRow>> {
        let row = sqlx::query_as::<_, CostCalibrationRow>(
            "SELECT form, coeff_a, coeff_b, sample_count, avg_error_pct, fitted_at
             FROM cost_calibration WHERE form = $1",
        )
        .bind(form)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Upsert cost calibration coefficients for a form.
    ///
    /// Called after fitting the power-law model to completed work block data.
    /// The `sample_count` and `avg_error_pct` provide confidence metadata.
    pub async fn upsert_cost_calibration(
        &self,
        form: &str,
        coeff_a: f64,
        coeff_b: f64,
        sample_count: i64,
        avg_error_pct: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO cost_calibration (form, coeff_a, coeff_b, sample_count, avg_error_pct, fitted_at)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (form) DO UPDATE SET
               coeff_a = EXCLUDED.coeff_a,
               coeff_b = EXCLUDED.coeff_b,
               sample_count = EXCLUDED.sample_count,
               avg_error_pct = EXCLUDED.avg_error_pct,
               fitted_at = NOW()",
        )
        .bind(form)
        .bind(coeff_a)
        .bind(coeff_b)
        .bind(sample_count)
        .bind(avg_error_pct)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
