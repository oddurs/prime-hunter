//! World record tracking operations.
//!
//! The `records` table stores known world records for each prime form/category,
//! scraped from sources like Chris Caldwell's Top 5000, OEIS, and PrimePages.
//! Each record includes the current holder, our best prime for comparison,
//! and the percentage gap to the record.
//!
//! Records are upserted on (form, category) so repeated scraping updates
//! existing entries rather than creating duplicates.

use super::Database;
use anyhow::Result;

impl Database {
    /// Upsert a world record entry (insert or update on form+category).
    ///
    /// The `discovered_at` string is parsed flexibly: "2024", "2024-01-15",
    /// or "Jan 2024" are all accepted. Unparseable dates are silently dropped.
    pub async fn upsert_record(
        &self,
        form: &str,
        category: &str,
        expression: &str,
        digits: i64,
        holder: Option<&str>,
        discovered_at: Option<&str>,
        source: Option<&str>,
        source_url: Option<&str>,
        our_best_id: Option<i64>,
        our_best_digits: i64,
    ) -> Result<()> {
        let disc_date = discovered_at.and_then(|d| {
            chrono::NaiveDate::parse_from_str(d, "%Y")
                .or_else(|_| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d"))
                .or_else(|_| chrono::NaiveDate::parse_from_str(d, "%b %Y"))
                .ok()
        });

        sqlx::query(
            "INSERT INTO records (form, category, expression, digits, holder, discovered_at,
                                  source, source_url, our_best_id, our_best_digits)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (form, category) DO UPDATE SET
               expression = EXCLUDED.expression,
               digits = EXCLUDED.digits,
               holder = EXCLUDED.holder,
               discovered_at = COALESCE(EXCLUDED.discovered_at, records.discovered_at),
               source = EXCLUDED.source,
               source_url = EXCLUDED.source_url,
               our_best_id = EXCLUDED.our_best_id,
               our_best_digits = EXCLUDED.our_best_digits,
               updated_at = NOW()",
        )
        .bind(form)
        .bind(category)
        .bind(expression)
        .bind(digits)
        .bind(holder)
        .bind(disc_date)
        .bind(source)
        .bind(source_url)
        .bind(our_best_id)
        .bind(our_best_digits)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all records with our-best comparison, ordered by form and category.
    pub async fn get_records(&self) -> Result<Vec<crate::project::RecordRow>> {
        let rows = sqlx::query_as::<_, crate::project::RecordRow>(
            "SELECT id, form, category, expression, digits, holder, discovered_at,
                    source, source_url, our_best_id, our_best_digits, fetched_at, updated_at
             FROM records ORDER BY form, category",
        )
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }
}
