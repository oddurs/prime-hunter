//! Project management API — CRUD, lifecycle, cost estimation, records.

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use super::AppState;
use crate::project;

#[derive(Deserialize)]
pub(super) struct ProjectListQuery {
    status: Option<String>,
}

pub(super) async fn handler_api_projects_list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProjectListQuery>,
) -> impl IntoResponse {
    match state.db.get_projects(params.status.as_deref()).await {
        Ok(projects) => Json(serde_json::json!(projects)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct CreateProjectPayload {
    name: String,
    description: Option<String>,
    objective: String,
    form: String,
    #[serde(default)]
    target: serde_json::Value,
    #[serde(default)]
    competitive: serde_json::Value,
    #[serde(default)]
    strategy: serde_json::Value,
    #[serde(default)]
    infrastructure: serde_json::Value,
    #[serde(default)]
    budget: serde_json::Value,
    #[serde(default)]
    workers: serde_json::Value,
}

/// POST /api/projects — Create a project from JSON.
pub(super) async fn handler_api_projects_create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateProjectPayload>,
) -> impl IntoResponse {
    let toml_str = format!(
        "[project]\nname = {:?}\ndescription = {:?}\nobjective = {:?}\nform = {:?}\n",
        payload.name,
        payload.description.as_deref().unwrap_or(""),
        payload.objective,
        payload.form,
    );

    let obj = match payload.objective.as_str() {
        "record" => project::Objective::Record,
        "survey" => project::Objective::Survey,
        "verification" => project::Objective::Verification,
        "custom" => project::Objective::Custom,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid objective: {}", other)})),
            )
                .into_response();
        }
    };

    let target: project::TargetConfig =
        serde_json::from_value(payload.target.clone()).unwrap_or_default();
    let strategy: project::StrategyConfig =
        serde_json::from_value(payload.strategy.clone()).unwrap_or_default();

    let config = project::ProjectConfig {
        project: project::ProjectMeta {
            name: payload.name.clone(),
            description: payload.description.unwrap_or_default(),
            objective: obj,
            form: payload.form.clone(),
            author: String::new(),
            tags: vec![],
        },
        target,
        competitive: serde_json::from_value(payload.competitive).ok(),
        strategy,
        infrastructure: serde_json::from_value(payload.infrastructure).ok(),
        budget: serde_json::from_value(payload.budget).ok(),
        workers: serde_json::from_value(payload.workers).ok(),
    };

    match state.db.create_project(&config, Some(&toml_str)).await {
        Ok(id) => {
            let slug = project::slugify(&payload.name);
            eprintln!(
                "Project '{}' created (id={}, slug={})",
                payload.name, id, slug
            );
            (
                StatusCode::CREATED,
                Json(serde_json::json!({"id": id, "slug": slug})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct ImportTomlPayload {
    toml: String,
}

/// POST /api/projects/import — Import a project from TOML content.
pub(super) async fn handler_api_projects_import(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ImportTomlPayload>,
) -> impl IntoResponse {
    let config = match project::parse_toml(&payload.toml) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("TOML parse error: {}", e)})),
            )
                .into_response();
        }
    };

    match state.db.create_project(&config, Some(&payload.toml)).await {
        Ok(id) => {
            let slug = project::slugify(&config.project.name);
            eprintln!(
                "Project '{}' imported (id={}, slug={})",
                config.project.name, id, slug
            );
            (
                StatusCode::CREATED,
                Json(serde_json::json!({"id": id, "slug": slug})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/projects/{slug} — Get project details with phases and recent events.
pub(super) async fn handler_api_project_get(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let phases = state
        .db
        .get_project_phases(proj.id)
        .await
        .unwrap_or_default();
    let events = state
        .db
        .get_project_events(proj.id, 50)
        .await
        .unwrap_or_default();

    Json(serde_json::json!({
        "project": proj,
        "phases": phases,
        "events": events,
    }))
    .into_response()
}

/// POST /api/projects/{slug}/activate — Start project orchestration.
pub(super) async fn handler_api_project_activate(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    if proj.status != "draft" && proj.status != "paused" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Cannot activate project with status '{}'", proj.status)
            })),
        )
            .into_response();
    }

    if let Err(e) = state.db.update_project_status(proj.id, "active").await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    state
        .db
        .insert_project_event(
            proj.id,
            "activated",
            &format!("Project '{}' activated via API", proj.name),
            None,
        )
        .await
        .ok();

    eprintln!("Project '{}' activated via API", slug);
    Json(serde_json::json!({"ok": true, "status": "active"})).into_response()
}

/// POST /api/projects/{slug}/pause — Pause project orchestration.
pub(super) async fn handler_api_project_pause(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    if proj.status != "active" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Cannot pause project with status '{}'", proj.status)
            })),
        )
            .into_response();
    }

    if let Err(e) = state.db.update_project_status(proj.id, "paused").await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    state
        .db
        .insert_project_event(
            proj.id,
            "paused",
            &format!("Project '{}' paused via API", proj.name),
            None,
        )
        .await
        .ok();

    Json(serde_json::json!({"ok": true, "status": "paused"})).into_response()
}

/// POST /api/projects/{slug}/cancel — Cancel a project.
pub(super) async fn handler_api_project_cancel(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    if let Err(e) = state.db.update_project_status(proj.id, "cancelled").await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    state
        .db
        .insert_project_event(
            proj.id,
            "cancelled",
            &format!("Project '{}' cancelled via API", proj.name),
            None,
        )
        .await
        .ok();

    Json(serde_json::json!({"ok": true, "status": "cancelled"})).into_response()
}

/// GET /api/projects/{slug}/events — Get project activity log.
pub(super) async fn handler_api_project_events(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    match state.db.get_project_events(proj.id, 100).await {
        Ok(events) => Json(serde_json::json!(events)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/projects/{slug}/cost — Get cost estimate for a project.
pub(super) async fn handler_api_project_cost(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
) -> impl IntoResponse {
    let proj = match state.db.get_project_by_slug(&slug).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let config = if let Some(toml_src) = &proj.toml_source {
        match project::parse_toml(toml_src) {
            Ok(c) => c,
            Err(_) => {
                return Json(serde_json::json!({"error": "Invalid stored TOML"})).into_response()
            }
        }
    } else {
        project::ProjectConfig {
            project: project::ProjectMeta {
                name: proj.name.clone(),
                description: proj.description.clone(),
                objective: match proj.objective.as_str() {
                    "record" => project::Objective::Record,
                    "survey" => project::Objective::Survey,
                    "verification" => project::Objective::Verification,
                    _ => project::Objective::Custom,
                },
                form: proj.form.clone(),
                author: String::new(),
                tags: vec![],
            },
            target: serde_json::from_value(proj.target.clone()).unwrap_or_default(),
            competitive: serde_json::from_value(proj.competitive.clone()).ok(),
            strategy: serde_json::from_value(proj.strategy.clone()).unwrap_or_default(),
            infrastructure: serde_json::from_value(proj.infrastructure.clone()).ok(),
            budget: serde_json::from_value(proj.budget.clone()).ok(),
            workers: None,
        }
    };

    let estimate = project::estimate_project_cost(&config);
    Json(serde_json::json!(estimate)).into_response()
}

// ── Records Endpoints ───────────────────────────────────────────

/// GET /api/records — Get all world records with our-best comparison.
pub(super) async fn handler_api_records(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_records().await {
        Ok(records) => Json(serde_json::json!(records)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/records/refresh — Trigger manual records refresh from t5k.org.
pub(super) async fn handler_api_records_refresh(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match project::refresh_all_records(&state.db).await {
        Ok(n) => {
            eprintln!("Records manually refreshed: {} forms updated", n);
            Json(serde_json::json!({"ok": true, "updated": n})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
