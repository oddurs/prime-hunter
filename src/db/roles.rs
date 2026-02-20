//! Agent role operations.
//!
//! Roles provide domain-specific configuration for agents: system prompt,
//! default model, permission level, and associated templates. Built-in roles
//! include "engine", "frontend", "ops", and "research", each scoped to a
//! set of domains with appropriate defaults.

use anyhow::Result;
use super::{Database, AgentRoleRow, AgentTemplateRow};

impl Database {
    /// Retrieve all agent roles, ordered by name.
    pub async fn get_all_roles(&self) -> Result<Vec<AgentRoleRow>> {
        let rows = sqlx::query_as::<_, AgentRoleRow>(
            "SELECT id, name, description, domains, default_permission_level, default_model,
                    system_prompt, default_max_cost_usd::FLOAT8 AS default_max_cost_usd,
                    created_at, updated_at
             FROM agent_roles ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Retrieve a single role by name.
    pub async fn get_role_by_name(&self, name: &str) -> Result<Option<AgentRoleRow>> {
        let row = sqlx::query_as::<_, AgentRoleRow>(
            "SELECT id, name, description, domains, default_permission_level, default_model,
                    system_prompt, default_max_cost_usd::FLOAT8 AS default_max_cost_usd,
                    created_at, updated_at
             FROM agent_roles WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get templates associated with a role via the junction table.
    ///
    /// Returns templates linked through `agent_role_templates` (many-to-many).
    pub async fn get_role_templates(&self, role_name: &str) -> Result<Vec<AgentTemplateRow>> {
        let rows = sqlx::query_as::<_, AgentTemplateRow>(
            "SELECT t.id, t.name, t.description, t.steps, t.created_at, t.role_name
             FROM agent_templates t
             JOIN agent_role_templates rt ON rt.template_name = t.name
             WHERE rt.role_name = $1
             ORDER BY t.name",
        )
        .bind(role_name)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
