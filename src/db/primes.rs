//! Prime record CRUD operations — insert, query, filter, verify, and count.
//!
//! This module handles all database operations for the `primes` table: inserting
//! newly discovered primes (both async and sync-from-rayon), filtered listing with
//! dynamic WHERE clauses, verification status updates, and best-per-form lookups.

use super::{Database, PrimeDetail, PrimeFilter, PrimeRecord};
use anyhow::Result;

impl Database {
    /// Insert a new prime record with the current timestamp.
    ///
    /// Called from engine modules after a candidate passes primality testing.
    /// The `certificate` parameter is an optional JSON string containing the
    /// primality certificate (Proth witness, LLR residue, Pocklington chain, etc.).
    pub async fn insert_prime(
        &self,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
        certificate: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO primes (form, expression, digits, found_at, search_params, proof_method, certificate)
             VALUES ($1, $2, $3, NOW(), $4, $5, $6::jsonb)",
        )
        .bind(form)
        .bind(expression)
        .bind(digits as i64)
        .bind(search_params)
        .bind(proof_method)
        .bind(certificate)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a prime, ignoring duplicates on (form, expression).
    ///
    /// Used during bulk imports or re-verification where the same prime may
    /// already exist in the database.
    pub async fn insert_prime_ignore(
        &self,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO primes (form, expression, digits, found_at, search_params, proof_method)
             VALUES ($1, $2, $3, NOW(), $4, $5)
             ON CONFLICT (form, expression) DO NOTHING",
        )
        .bind(form)
        .bind(expression)
        .bind(digits as i64)
        .bind(search_params)
        .bind(proof_method)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Synchronous wrapper for engine modules running on rayon threads.
    ///
    /// Engine modules execute inside `rayon::par_iter` closures which cannot use
    /// `.await`. This bridges async sqlx operations into sync context via
    /// `tokio::runtime::Handle::block_on`. Safe because rayon threads are not
    /// tokio tasks — they won't deadlock the executor.
    pub fn insert_prime_sync(
        &self,
        rt: &tokio::runtime::Handle,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
        certificate: Option<&str>,
    ) -> Result<()> {
        rt.block_on(self.insert_prime(
            form,
            expression,
            digits,
            search_params,
            proof_method,
            certificate,
        ))
    }

    /// Synchronous duplicate-ignoring insert for rayon threads.
    pub fn insert_prime_ignore_sync(
        &self,
        rt: &tokio::runtime::Handle,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
        proof_method: &str,
    ) -> Result<()> {
        rt.block_on(self.insert_prime_ignore(form, expression, digits, search_params, proof_method))
    }

    /// Query primes with dynamic filtering, sorting, and pagination.
    ///
    /// Builds a parameterized SQL query at runtime based on which filter fields
    /// are set. Sort column and direction are whitelist-validated by `PrimeFilter`
    /// methods to prevent SQL injection.
    pub async fn get_primes_filtered(
        &self,
        limit: i64,
        offset: i64,
        filter: &PrimeFilter,
    ) -> Result<Vec<PrimeRecord>> {
        let mut conditions = Vec::new();
        let mut param_idx = 1u32;

        if filter.form.is_some() {
            conditions.push(format!("form = ${}", param_idx));
            param_idx += 1;
        }
        if filter.search.is_some() {
            conditions.push(format!("expression LIKE ${}", param_idx));
            param_idx += 1;
        }
        if filter.min_digits.is_some() {
            conditions.push(format!("digits >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.max_digits.is_some() {
            conditions.push(format!("digits <= ${}", param_idx));
            param_idx += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, form, expression, digits, found_at, proof_method FROM primes{} ORDER BY {} {} LIMIT ${} OFFSET ${}",
            where_clause,
            filter.safe_sort_column(),
            filter.safe_sort_dir(),
            param_idx,
            param_idx + 1,
        );

        let mut query = sqlx::query_as::<_, PrimeRecord>(&sql);
        if let Some(ref form) = filter.form {
            query = query.bind(form);
        }
        if let Some(ref search) = filter.search {
            query = query.bind(format!("%{}%", search));
        }
        if let Some(min_d) = filter.min_digits {
            query = query.bind(min_d);
        }
        if let Some(max_d) = filter.max_digits {
            query = query.bind(max_d);
        }
        query = query.bind(limit);
        query = query.bind(offset);

        let records = query.fetch_all(&self.read_pool).await?;
        Ok(records)
    }

    /// Count primes matching the given filter (for pagination metadata).
    pub async fn get_filtered_count(&self, filter: &PrimeFilter) -> Result<i64> {
        let mut conditions = Vec::new();
        let mut param_idx = 1u32;

        if filter.form.is_some() {
            conditions.push(format!("form = ${}", param_idx));
            param_idx += 1;
        }
        if filter.search.is_some() {
            conditions.push(format!("expression LIKE ${}", param_idx));
            param_idx += 1;
        }
        if filter.min_digits.is_some() {
            conditions.push(format!("digits >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.max_digits.is_some() {
            conditions.push(format!("digits <= ${}", param_idx));
            param_idx += 1;
        }
        let _ = param_idx;

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT COUNT(*) as count FROM primes{}", where_clause);

        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        if let Some(ref form) = filter.form {
            query = query.bind(form);
        }
        if let Some(ref search) = filter.search {
            query = query.bind(format!("%{}%", search));
        }
        if let Some(min_d) = filter.min_digits {
            query = query.bind(min_d);
        }
        if let Some(max_d) = filter.max_digits {
            query = query.bind(max_d);
        }

        let count = query.fetch_one(&self.read_pool).await?;
        Ok(count)
    }

    /// Get unverified primes for the verification pipeline.
    pub async fn get_unverified_primes(&self, limit: i64) -> Result<Vec<PrimeDetail>> {
        let rows = sqlx::query_as::<_, PrimeDetail>(
            "SELECT id, form, expression, digits, found_at, search_params, proof_method
             FROM primes WHERE NOT verified ORDER BY id LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    /// Get unverified primes with optional form filter and force-reverify flag.
    ///
    /// When `force` is true, returns already-verified primes too (for re-verification).
    pub async fn get_unverified_primes_filtered(
        &self,
        limit: i64,
        form: Option<&str>,
        force: bool,
    ) -> Result<Vec<PrimeDetail>> {
        let verified_clause = if force { "" } else { "NOT verified AND" };
        let (sql, has_form) = if form.is_some() {
            (
                format!(
                    "SELECT id, form, expression, digits, found_at, search_params, proof_method
                     FROM primes WHERE {} form = $1 ORDER BY id LIMIT $2",
                    verified_clause
                ),
                true,
            )
        } else {
            (
                format!(
                    "SELECT id, form, expression, digits, found_at, search_params, proof_method
                     FROM primes WHERE {} TRUE ORDER BY id LIMIT $1",
                    verified_clause
                ),
                false,
            )
        };
        let mut query = sqlx::query_as::<_, PrimeDetail>(&sql);
        if has_form {
            query = query.bind(form.unwrap());
            query = query.bind(limit);
        } else {
            query = query.bind(limit);
        }
        let rows = query.fetch_all(&self.read_pool).await?;
        Ok(rows)
    }

    /// Get a single prime by ID with full detail (including search_params).
    pub async fn get_prime_by_id(&self, id: i64) -> Result<Option<PrimeDetail>> {
        let row = sqlx::query_as::<_, PrimeDetail>(
            "SELECT id, form, expression, digits, found_at, search_params, proof_method
             FROM primes WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Mark a prime as verified with the verification method and tier.
    ///
    /// Tier levels: 1 = deterministic proof, 2 = BPSW+MR10, 3 = PFGW cross-verify.
    pub async fn mark_verified(&self, id: i64, method: &str, tier: i16) -> Result<()> {
        sqlx::query(
            "UPDATE primes SET verified = true, verified_at = NOW(), verification_method = $1, verification_tier = $2 WHERE id = $3",
        )
        .bind(method)
        .bind(tier)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark a prime's verification as failed with a reason string.
    pub async fn mark_verification_failed(&self, id: i64, reason: &str) -> Result<()> {
        sqlx::query(
            "UPDATE primes SET verification_method = $1, verification_tier = 0 WHERE id = $2",
        )
        .bind(reason)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get our largest prime for a given form (used for records comparison).
    pub async fn get_best_prime_for_form(&self, form: &str) -> Result<Option<PrimeRecord>> {
        let row = sqlx::query_as::<_, PrimeRecord>(
            "SELECT id, form, expression, digits, found_at, proof_method
             FROM primes WHERE form = $1 ORDER BY digits DESC LIMIT 1",
        )
        .bind(form)
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }

    /// Count primes discovered within a time range.
    pub async fn count_primes_in_range(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM primes WHERE found_at BETWEEN $1 AND $2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.read_pool)
        .await?;
        Ok(count)
    }

    /// Count primes per form within a time range.
    pub async fn count_primes_by_form_in_range(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<(String, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT form, COUNT(*)::BIGINT FROM primes WHERE found_at BETWEEN $1 AND $2 GROUP BY form ORDER BY form",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }
}
