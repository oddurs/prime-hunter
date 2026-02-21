//! JWT auth middleware for dashboard API routes.
//!
//! Extracts the Supabase JWT from the `Authorization: Bearer <token>` header,
//! decodes it, and looks up the user's role from `user_profiles`. The role is
//! injected into request extensions as `AuthUser` for downstream handlers.
//!
//! Admin-only routes use the `RequireAdmin` extractor to gate access.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::AppState;

/// JWT claims from a Supabase-issued token.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SupabaseClaims {
    /// Subject — the Supabase auth user ID (UUID).
    sub: String,
    /// Role claim from Supabase (e.g. "authenticated").
    #[serde(default)]
    role: String,
}

/// Authenticated user info, injected into request extensions.
#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub user_id: String,
    pub role: String,
}

/// Extract the JWT secret from environment (Supabase JWT secret).
fn jwt_secret() -> Option<String> {
    std::env::var("SUPABASE_JWT_SECRET").ok()
}

/// Decode and optionally verify a Supabase JWT.
///
/// If `SUPABASE_JWT_SECRET` is set, performs full HS256 verification.
/// Otherwise, decodes without signature validation (development mode).
fn decode_jwt(token: &str) -> Result<SupabaseClaims, String> {
    if let Some(secret) = jwt_secret() {
        let key = DecodingKey::from_secret(secret.as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["authenticated"]);
        let data = decode::<SupabaseClaims>(token, &key, &validation)
            .map_err(|e| format!("JWT verification failed: {}", e))?;
        Ok(data.claims)
    } else {
        // Development mode: decode without verification
        let mut validation = Validation::new(Algorithm::HS256);
        validation.insecure_disable_signature_validation();
        validation.set_audience(&["authenticated"]);
        validation.validate_exp = false;
        let data = decode::<SupabaseClaims>(token, &DecodingKey::from_secret(b""), &validation)
            .map_err(|e| format!("JWT decode failed: {}", e))?;
        Ok(data.claims)
    }
}

/// Middleware function to extract auth info from the request.
///
/// Call this from handlers that need optional auth. For required auth,
/// use the `RequireAdmin` extractor instead.
pub async fn extract_auth_user(
    state: &Arc<AppState>,
    parts: &Parts,
) -> Option<AuthUser> {
    let auth_header = parts
        .headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;

    let token = auth_header.strip_prefix("Bearer ")?;
    let claims = decode_jwt(token).ok()?;

    // Look up role from user_profiles (defaults to "operator" if no profile)
    let role = state
        .db
        .get_user_role(&claims.sub)
        .await
        .unwrap_or_else(|_| "operator".to_string());

    Some(AuthUser {
        user_id: claims.sub,
        role,
    })
}

/// Axum extractor that requires an authenticated admin user.
///
/// Returns 401 if no valid JWT is present, 403 if the user is not an admin.
#[allow(dead_code)]
pub struct RequireAdmin(pub AuthUser);

impl FromRequestParts<Arc<AppState>> for RequireAdmin {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = extract_auth_user(state, parts).await.ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Authentication required"})),
            )
                .into_response()
        })?;

        if auth_user.role != "admin" {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "Admin access required"})),
            )
                .into_response());
        }

        Ok(RequireAdmin(auth_user))
    }
}

/// Axum extractor that requires any authenticated user.
///
/// Returns 401 if no valid JWT is present.
pub struct RequireAuth(pub AuthUser);

impl FromRequestParts<Arc<AppState>> for RequireAuth {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = extract_auth_user(state, parts).await.ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Authentication required"})),
            )
                .into_response()
        })?;

        Ok(RequireAuth(auth_user))
    }
}
