//! Agent memory CRUD operations.
//!
//! Agent memory provides a key-value store for agents to persist learnings,
//! observations, and intermediate results across task executions. Entries are
//! categorized (e.g., "strategy", "observation", "config") and linked to the
//! task that created them.

use super::{AgentMemoryRow, Database};
use anyhow::Result;

impl Database {
    /// Retrieve all agent memory entries, ordered by category then key.
    pub async fn get_all_agent_memory(&self) -> Result<Vec<AgentMemoryRow>> {
        let rows = sqlx::query_as::<_, AgentMemoryRow>(
            "SELECT id, key, value, category, created_by_task, created_at, updated_at
             FROM agent_memory ORDER BY category, key",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Retrieve agent memory entries filtered by category.
    pub async fn get_agent_memory_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<AgentMemoryRow>> {
        let rows = sqlx::query_as::<_, AgentMemoryRow>(
            "SELECT id, key, value, category, created_by_task, created_at, updated_at
             FROM agent_memory WHERE category = $1 ORDER BY key",
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Upsert an agent memory entry.
    ///
    /// If the key already exists, updates value, category, and task reference.
    /// Uses PostgreSQL `ON CONFLICT (key) DO UPDATE` for atomic upsert.
    pub async fn upsert_agent_memory(
        &self,
        key: &str,
        value: &str,
        category: &str,
        task_id: Option<i64>,
    ) -> Result<AgentMemoryRow> {
        let row = sqlx::query_as::<_, AgentMemoryRow>(
            "INSERT INTO agent_memory (key, value, category, created_by_task)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (key) DO UPDATE SET
               value = EXCLUDED.value,
               category = EXCLUDED.category,
               created_by_task = EXCLUDED.created_by_task,
               updated_at = now()
             RETURNING id, key, value, category, created_by_task, created_at, updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(category)
        .bind(task_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Delete an agent memory entry by key. Returns true if a row was deleted.
    pub async fn delete_agent_memory(&self, key: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM agent_memory WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
