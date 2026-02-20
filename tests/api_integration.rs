//! API integration tests using tower::ServiceExt::oneshot.
//!
//! All tests require TEST_DATABASE_URL to be set.
//! Run with: TEST_DATABASE_URL=postgres://... cargo test --test api_integration
//!
//! Tests should be run single-threaded to avoid conflicts:
//!   cargo test --test api_integration -- --test-threads=1

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Skip the test if TEST_DATABASE_URL is not set.
macro_rules! require_db {
    () => {
        if !common::has_test_db() {
            eprintln!("Skipping: TEST_DATABASE_URL not set");
            return;
        }
    };
}

async fn app() -> Router {
    common::build_test_app().await
}

async fn get(app: Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::json!(null));
    (status, json)
}

async fn post_json(
    app: Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!(null));
    (status, json)
}

// --- Status and Info endpoints ---

#[tokio::test]
async fn get_status_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/status").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("active").is_some());
}

#[tokio::test]
async fn get_fleet_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/fleet").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("workers").is_some());
    assert_eq!(json["total_workers"], 0);
}

#[tokio::test]
async fn get_searches_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/searches").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("searches").is_some());
}

#[tokio::test]
async fn get_events_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/events").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("events").is_some());
}

#[tokio::test]
async fn get_notifications_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/notifications").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("notifications").is_some());
}

#[tokio::test]
async fn get_docs_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/docs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("docs").is_some());
}

#[tokio::test]
async fn get_volunteer_worker_latest_returns_channel_release() {
    require_db!();
    let (status, json) = get(app().await, "/api/volunteer/worker/latest?channel=stable").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["channel"], "stable");
    assert!(json["version"].is_string());
    assert!(json["artifacts"].is_array());
}

#[tokio::test]
async fn releases_rollout_and_latest_from_db() {
    require_db!();
    let router = app().await;

    let artifacts = serde_json::json!([{
        "os": "linux",
        "arch": "x86_64",
        "url": "https://example.invalid/darkreach-worker-linux-x86_64.tar.gz",
        "sha256": "deadbeef"
    }]);

    let (status, json) = post_json(
        router.clone(),
        "/api/releases/worker",
        serde_json::json!({
            "version": "9.9.8-test",
            "artifacts": artifacts,
            "notes": "test release old"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["release"]["version"], "9.9.8-test");

    let (status, json) = post_json(
        router.clone(),
        "/api/releases/worker",
        serde_json::json!({
            "version": "9.9.9-test",
            "artifacts": serde_json::json!([{
                "os": "linux",
                "arch": "x86_64",
                "url": "https://example.invalid/darkreach-worker-linux-x86_64-v2.tar.gz",
                "sha256": "beefdead"
            }]),
            "notes": "test release new"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["release"]["version"], "9.9.9-test");

    let (status, json) = post_json(
        router.clone(),
        "/api/releases/rollout",
        serde_json::json!({
            "channel": "stable",
            "version": "9.9.8-test",
            "rollout_percent": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["channel"]["version"], "9.9.8-test");

    let (status, json) = post_json(
        router.clone(),
        "/api/releases/rollout",
        serde_json::json!({
            "channel": "stable",
            "version": "9.9.9-test",
            "rollout_percent": 0
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["channel"]["version"], "9.9.9-test");

    let (status, json) = get(
        router.clone(),
        "/api/volunteer/worker/latest?channel=stable&worker_id=abc",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["version"], "9.9.8-test");

    let (status, json) = post_json(
        router.clone(),
        "/api/releases/rollout",
        serde_json::json!({
            "channel": "stable",
            "version": "9.9.9-test",
            "rollout_percent": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["channel"]["version"], "9.9.9-test");

    let (status, json) = get(
        router,
        "/api/volunteer/worker/latest?channel=stable&worker_id=abc",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["version"], "9.9.9-test");
}

// --- Worker API ---

#[tokio::test]
async fn post_worker_register_and_heartbeat() {
    require_db!();
    let router = app().await;

    // Register
    let (status, json) = post_json(
        router.clone(),
        "/api/worker/register",
        serde_json::json!({
            "worker_id": "api-test-worker",
            "hostname": "test-host",
            "cores": 8,
            "search_type": "factorial",
            "search_params": "{\"start\":1,\"end\":100}"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ok"], true);

    // Heartbeat
    let (status, json) = post_json(
        router.clone(),
        "/api/worker/heartbeat",
        serde_json::json!({
            "worker_id": "api-test-worker",
            "tested": 42,
            "found": 3,
            "current": "testing n=50"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ok"], true);

    // Fleet should now show the worker
    let (status, json) = get(router.clone(), "/api/fleet").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["total_workers"].as_i64().unwrap() >= 1);

    // Deregister
    let (status, json) = post_json(
        router.clone(),
        "/api/worker/deregister",
        serde_json::json!({"worker_id": "api-test-worker"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ok"], true);
}

#[tokio::test]
async fn post_worker_prime() {
    require_db!();
    let router = app().await;

    let (status, json) = post_json(
        router,
        "/api/worker/prime",
        serde_json::json!({
            "form": "factorial",
            "expression": "5! + 1",
            "digits": 3,
            "search_params": "{}",
            "proof_method": "deterministic"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ok"], true);
}

// --- Search Job API ---

#[tokio::test]
async fn get_search_jobs_empty() {
    require_db!();
    let (status, json) = get(app().await, "/api/search_jobs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["search_jobs"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn post_search_job_creates_blocks() {
    require_db!();
    let router = app().await;

    let (status, json) = post_json(
        router.clone(),
        "/api/search_jobs",
        serde_json::json!({
            "search_type": "factorial",
            "params": {"start": 1, "end": 500},
            "range_start": 1,
            "range_end": 500,
            "block_size": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(json.get("id").is_some());
    assert_eq!(json["blocks"], 5);

    // Verify it appears in the list
    let (status, json) = get(router.clone(), "/api/search_jobs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["search_jobs"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn post_search_job_validates_range() {
    require_db!();
    let router = app().await;

    // range_start >= range_end should fail
    let (status, json) = post_json(
        router.clone(),
        "/api/search_jobs",
        serde_json::json!({
            "search_type": "factorial",
            "params": {},
            "range_start": 500,
            "range_end": 100,
            "block_size": 10
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn get_search_job_detail() {
    require_db!();
    let router = app().await;

    // Create a job first
    let (_, create_json) = post_json(
        router.clone(),
        "/api/search_jobs",
        serde_json::json!({
            "search_type": "kbn",
            "params": {"k": 3, "base": 2},
            "range_start": 1,
            "range_end": 100,
            "block_size": 50
        }),
    )
    .await;
    let job_id = create_json["id"].as_i64().unwrap();

    // Get detail
    let (status, json) = get(router.clone(), &format!("/api/search_jobs/{}", job_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["job"]["search_type"], "kbn");
    assert!(json.get("blocks").is_some());
}

#[tokio::test]
async fn cancel_search_job() {
    require_db!();
    let router = app().await;

    let (_, create_json) = post_json(
        router.clone(),
        "/api/search_jobs",
        serde_json::json!({
            "search_type": "factorial",
            "params": {},
            "range_start": 1,
            "range_end": 100,
            "block_size": 10
        }),
    )
    .await;
    let job_id = create_json["id"].as_i64().unwrap();

    let (status, json) = post_json(
        router.clone(),
        &format!("/api/search_jobs/{}/cancel", job_id),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ok"], true);

    // Verify status changed
    let (_, detail) = get(router.clone(), &format!("/api/search_jobs/{}", job_id)).await;
    assert_eq!(detail["job"]["status"], "cancelled");
}

// --- Agent API ---

#[tokio::test]
async fn post_agent_task_and_retrieve() {
    require_db!();
    let router = app().await;

    let (status, json) = post_json(
        router.clone(),
        "/api/agents/tasks",
        serde_json::json!({
            "title": "Test integration task",
            "description": "Testing from integration test",
            "priority": "high",
            "agent_model": "opus",
            "source": "automated"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["title"], "Test integration task");
    assert_eq!(json["status"], "pending");

    let task_id = json["id"].as_i64().unwrap();

    // Get the task
    let (status, json) = get(router.clone(), &format!("/api/agents/tasks/{}", task_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["title"], "Test integration task");

    // List tasks
    let (status, json) = get(router.clone(), "/api/agents/tasks").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["tasks"].as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn get_agent_events() {
    require_db!();
    let (status, json) = get(app().await, "/api/agents/events").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("events").is_some());
}

#[tokio::test]
async fn get_agent_budgets() {
    require_db!();
    let (status, json) = get(app().await, "/api/agents/budgets").await;
    assert_eq!(status, StatusCode::OK);
    // Should have the 3 seeded budgets (daily, weekly, monthly)
    assert_eq!(json["budgets"].as_array().unwrap().len(), 3);
}

// --- Middleware tests ---

#[tokio::test]
async fn cors_headers_present() {
    require_db!();
    let router = app().await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .header("origin", "http://example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get("access-control-allow-origin")
        .is_some());
}

#[tokio::test]
async fn body_limit_enforced() {
    require_db!();
    let router = app().await;

    // Send a body larger than 1MB
    let large_body = "x".repeat(2 * 1024 * 1024);
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/worker/register")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(large_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

// --- Error cases ---

#[tokio::test]
async fn search_job_not_found() {
    require_db!();
    let (status, json) = get(app().await, "/api/search_jobs/99999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn doc_not_found() {
    require_db!();
    let (status, json) = get(app().await, "/api/docs/nonexistent-slug").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn doc_slug_rejects_path_traversal() {
    require_db!();
    let (status, _) = get(app().await, "/api/docs/../../../etc/passwd").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
