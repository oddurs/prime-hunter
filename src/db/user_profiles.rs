//! User profile queries — role lookup, profile CRUD.

use anyhow::Result;
use serde::Serialize;

use super::Database;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserProfile {
    pub id: String,
    pub role: String,
    pub operator_id: Option<uuid::Uuid>,
    pub display_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Database {
    /// Look up a user profile by Supabase auth user ID.
    pub async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        let row = sqlx::query_as::<_, UserProfile>(
            "SELECT id::text, role, operator_id, display_name, created_at, updated_at
             FROM user_profiles WHERE id = $1::uuid",
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row)
    }

    /// Get the role for a user (returns "operator" as default if no profile exists).
    pub async fn get_user_role(&self, user_id: &str) -> Result<String> {
        let role = sqlx::query_scalar::<_, String>(
            "SELECT role FROM user_profiles WHERE id = $1::uuid",
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(role.unwrap_or_else(|| "operator".to_string()))
    }

    /// Update a user's display name.
    pub async fn update_user_display_name(
        &self,
        user_id: &str,
        display_name: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE user_profiles SET display_name = $2, updated_at = NOW()
             WHERE id = $1::uuid",
        )
        .bind(user_id)
        .bind(display_name)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Link a user profile to an operator account.
    pub async fn link_user_to_operator(
        &self,
        user_id: &str,
        operator_id: uuid::Uuid,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE user_profiles SET operator_id = $2, updated_at = NOW()
             WHERE id = $1::uuid",
        )
        .bind(user_id)
        .bind(operator_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
