//! Project management operations.
//!
//! Projects are multi-phase search campaigns configured via TOML. Each project
//! has ordered phases that can depend on each other, with activation and completion
//! conditions controlling automatic progression.
//!
//! ## Lifecycle
//!
//! 1. `create_project` — parses TOML config, auto-generates strategy if requested,
//!    inserts project + phases in a transaction
//! 2. Orchestration tick activates phases when conditions are met
//! 3. Each phase links to a search job for execution
//! 4. `update_project_progress` / `update_project_cost` track aggregated stats
//! 5. Completion/failure transitions via `update_project_status`

use anyhow::Result;
use super::Database;

impl Database {
    /// Create a new project with phases from a parsed TOML configuration.
    ///
    /// If `auto_strategy` is enabled and no explicit phases are provided,
    /// generates phases automatically based on form, objective, and target.
    pub async fn create_project(
        &self,
        config: &crate::project::ProjectConfig,
        toml_source: Option<&str>,
    ) -> Result<i64> {
        use crate::project;

        let slug = project::slugify(&config.project.name);
        let phases = if config.strategy.auto_strategy && config.strategy.phases.is_empty() {
            project::generate_auto_strategy(config)
        } else {
            config.strategy.phases.clone()
        };

        let mut tx = self.pool.begin().await?;

        let project_id: i64 = sqlx::query_scalar(
            "INSERT INTO projects (slug, name, description, objective, form, toml_source,
                                   target, competitive, strategy, infrastructure, budget)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             RETURNING id",
        )
        .bind(&slug)
        .bind(&config.project.name)
        .bind(&config.project.description)
        .bind(config.project.objective.to_string())
        .bind(&config.project.form)
        .bind(toml_source)
        .bind(serde_json::to_value(&config.target)?)
        .bind(serde_json::to_value(&config.competitive)?)
        .bind(serde_json::to_value(&config.strategy)?)
        .bind(serde_json::to_value(&config.infrastructure)?)
        .bind(serde_json::to_value(&config.budget)?)
        .fetch_one(&mut *tx)
        .await?;

        // Insert phases
        for (i, phase) in phases.iter().enumerate() {
            sqlx::query(
                "INSERT INTO project_phases
                    (project_id, name, description, phase_order, search_params,
                     block_size, depends_on, activation_condition, completion_condition)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(project_id)
            .bind(&phase.name)
            .bind(&phase.description)
            .bind(i as i32)
            .bind(&phase.search_params)
            .bind(phase.block_size.unwrap_or(1000))
            .bind(
                phase
                    .depends_on
                    .as_ref()
                    .cloned()
                    .unwrap_or_default()
                    .as_slice(),
            )
            .bind(phase.activation_condition.as_deref())
            .bind(&phase.completion)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(project_id)
    }

    /// List projects, optionally filtered by status.
    pub async fn get_projects(
        &self,
        status_filter: Option<&str>,
    ) -> Result<Vec<crate::project::ProjectRow>> {
        let rows = if let Some(status) = status_filter {
            sqlx::query_as::<_, crate::project::ProjectRow>(
                "SELECT id, slug, name, description, objective, form, status, toml_source,
                        target, competitive, strategy, infrastructure, budget,
                        total_tested, total_found, best_prime_id, best_digits,
                        total_core_hours::FLOAT8 AS total_core_hours,
                        total_cost_usd::FLOAT8 AS total_cost_usd,
                        created_at, started_at, completed_at, updated_at
                 FROM projects WHERE status = $1 ORDER BY created_at DESC",
            )
            .bind(status)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, crate::project::ProjectRow>(
                "SELECT id, slug, name, description, objective, form, status, toml_source,
                        target, competitive, strategy, infrastructure, budget,
                        total_tested, total_found, best_prime_id, best_digits,
                        total_core_hours::FLOAT8 AS total_core_hours,
                        total_cost_usd::FLOAT8 AS total_cost_usd,
                        created_at, started_at, completed_at, updated_at
                 FROM projects ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// Get a single project by slug.
    pub async fn get_project_by_slug(
        &self,
        slug: &str,
    ) -> Result<Option<crate::project::ProjectRow>> {
        let row = sqlx::query_as::<_, crate::project::ProjectRow>(
            "SELECT id, slug, name, description, objective, form, status, toml_source,
                    target, competitive, strategy, infrastructure, budget,
                    total_tested, total_found, best_prime_id, best_digits,
                    total_core_hours::FLOAT8 AS total_core_hours,
                    total_cost_usd::FLOAT8 AS total_cost_usd,
                    created_at, started_at, completed_at, updated_at
             FROM projects WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get all phases for a project, ordered by phase_order.
    pub async fn get_project_phases(
        &self,
        project_id: i64,
    ) -> Result<Vec<crate::project::ProjectPhaseRow>> {
        let rows = sqlx::query_as::<_, crate::project::ProjectPhaseRow>(
            "SELECT id, project_id, name, description, phase_order, status,
                    search_params, block_size, depends_on, activation_condition,
                    completion_condition, search_job_id,
                    total_tested, total_found, started_at, completed_at
             FROM project_phases WHERE project_id = $1 ORDER BY phase_order",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update a project's status (draft -> active, active -> paused, etc.).
    ///
    /// Sets `started_at` when transitioning to "active" and `completed_at` for
    /// terminal states (completed, cancelled, failed).
    pub async fn update_project_status(&self, project_id: i64, status: &str) -> Result<()> {
        let now = chrono::Utc::now();
        let started = if status == "active" {
            Some(now)
        } else {
            None
        };
        let completed = if matches!(status, "completed" | "cancelled" | "failed") {
            Some(now)
        } else {
            None
        };

        sqlx::query(
            "UPDATE projects SET status = $1, updated_at = NOW(),
                    started_at = COALESCE($2, started_at),
                    completed_at = COALESCE($3, completed_at)
             WHERE id = $4",
        )
        .bind(status)
        .bind(started)
        .bind(completed)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update aggregated progress counters on a project.
    pub async fn update_project_progress(
        &self,
        project_id: i64,
        total_tested: i64,
        total_found: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE projects SET total_tested = $1, total_found = $2, updated_at = NOW()
             WHERE id = $3",
        )
        .bind(total_tested)
        .bind(total_found)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update a phase's status.
    pub async fn update_phase_status(&self, phase_id: i64, status: &str) -> Result<()> {
        let completed = if matches!(status, "completed" | "skipped" | "failed") {
            Some(chrono::Utc::now())
        } else {
            None
        };

        sqlx::query(
            "UPDATE project_phases SET status = $1, completed_at = COALESCE($2, completed_at)
             WHERE id = $3",
        )
        .bind(status)
        .bind(completed)
        .bind(phase_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update phase progress counters.
    pub async fn update_phase_progress(
        &self,
        phase_id: i64,
        total_tested: i64,
        total_found: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE project_phases SET total_tested = $1, total_found = $2 WHERE id = $3",
        )
        .bind(total_tested)
        .bind(total_found)
        .bind(phase_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Activate a phase: set status to active, link the search job.
    pub async fn activate_phase(&self, phase_id: i64, search_job_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE project_phases SET status = 'active', search_job_id = $1, started_at = NOW()
             WHERE id = $2",
        )
        .bind(search_job_id)
        .bind(phase_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a new phase into an existing project at runtime.
    ///
    /// Used by adaptive phase generation to add follow-up phases after
    /// a completed phase indicates more work is needed (e.g., extending
    /// a range when no primes were found).
    pub async fn insert_phase(
        &self,
        project_id: i64,
        phase: &crate::project::PhaseConfig,
        phase_order: i32,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO project_phases
                (project_id, name, description, phase_order, search_params,
                 block_size, depends_on, activation_condition, completion_condition)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING id",
        )
        .bind(project_id)
        .bind(&phase.name)
        .bind(&phase.description)
        .bind(phase_order)
        .bind(&phase.search_params)
        .bind(phase.block_size.unwrap_or(1000))
        .bind(
            phase
                .depends_on
                .as_ref()
                .cloned()
                .unwrap_or_default()
                .as_slice(),
        )
        .bind(phase.activation_condition.as_deref())
        .bind(&phase.completion)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Insert a project event for the activity log.
    pub async fn insert_project_event(
        &self,
        project_id: i64,
        event_type: &str,
        summary: &str,
        detail: Option<&serde_json::Value>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO project_events (project_id, event_type, summary, detail)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(project_id)
        .bind(event_type)
        .bind(summary)
        .bind(detail)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get recent events for a project.
    pub async fn get_project_events(
        &self,
        project_id: i64,
        limit: i64,
    ) -> Result<Vec<crate::project::ProjectEventRow>> {
        let rows = sqlx::query_as::<_, crate::project::ProjectEventRow>(
            "SELECT id, project_id, event_type, summary, detail, created_at
             FROM project_events WHERE project_id = $1
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update a project's cost and core-hour totals.
    ///
    /// Called from the orchestration tick after computing actual cost from
    /// work block durations and the cost calibration model.
    pub async fn update_project_cost(
        &self,
        project_id: i64,
        total_core_hours: f64,
        total_cost_usd: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE projects SET total_core_hours = $1, total_cost_usd = $2, updated_at = NOW()
             WHERE id = $3",
        )
        .bind(total_core_hours)
        .bind(total_cost_usd)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update a project's best prime reference.
    ///
    /// Called from the orchestration tick when a new largest prime is found.
    pub async fn update_project_best_prime(
        &self,
        project_id: i64,
        best_prime_id: Option<i64>,
        best_digits: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE projects SET best_prime_id = $1, best_digits = $2, updated_at = NOW()
             WHERE id = $3",
        )
        .bind(best_prime_id)
        .bind(best_digits)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
