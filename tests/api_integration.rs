//! API integration tests for the darkreach Axum REST endpoints.
//!
//! These tests exercise every public HTTP route in the dashboard API using
//! `tower::ServiceExt::oneshot` to send synthetic requests directly to the
//! Axum router without starting a TCP listener. This approach is faster than
//! end-to-end HTTP tests and avoids port conflicts in CI.
//!
//! # Prerequisites
//!
//! - A running PostgreSQL instance with the `TEST_DATABASE_URL` environment variable set.
//! - Example: `TEST_DATABASE_URL=postgres://user:pass@localhost:5432/darkreach_test`
//!
//! # How to run
//!
//! ```bash
//! # Run all API integration tests (single-threaded to avoid table conflicts):
//! TEST_DATABASE_URL=postgres://... cargo test --test api_integration -- --test-threads=1
//!
//! # Run a specific test:
//! TEST_DATABASE_URL=postgres://... cargo test --test api_integration cors_headers_present
//! ```
//!
//! # Testing strategy
//!
//! Each test builds a fresh Axum router via `common::build_test_app()`, which
//! truncates all database tables and re-seeds reference data. Tests are grouped
//! by API domain: status/info endpoints, worker lifecycle, search job management,
//! agent task API, middleware behavior, error handling, and operator (volunteer) API.
//!
//! The helper functions `get()` and `post_json()` abstract away request construction
//! and response parsing, returning `(StatusCode, serde_json::Value)` tuples for
//! concise assertions.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Skip the test if TEST_DATABASE_URL is not set.
///
/// Provides a clean skip mechanism for environments without a test database.
/// Prints a diagnostic message to stderr and returns early.
macro_rules! require_db {
    () => {
        if !common::has_test_db() {
            eprintln!("Skipping: TEST_DATABASE_URL not set");
            return;
        }
    };
}

/// Builds a fresh Axum test router with a clean database.
///
/// Each call truncates all tables and re-seeds reference data, so every
/// test starts with a known-clean state.
async fn app() -> Router {
    common::build_test_app().await
}

/// Sends a GET request to the given URI and returns the status code and parsed JSON body.
///
/// If the response body is not valid JSON, returns `serde_json::json!(null)`.
/// This is a convenience wrapper that eliminates boilerplate request construction
/// across all GET-based test cases.
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

/// Sends a POST request with a JSON body and returns the status code and parsed response.
///
/// Sets `Content-Type: application/json` and serializes the provided `serde_json::Value`.
/// Used for all write operations (worker registration, job creation, task submission, etc.).
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

// == Status and Info Endpoints =================================================
// Smoke tests for read-only informational endpoints. These verify the API
// returns 200 OK with the expected JSON structure, even with an empty database.
// ==============================================================================

/// Verifies the /api/status endpoint returns 200 with an "active" field.
///
/// Exercises: GET /api/status, coordinator health check.
///
/// This endpoint is polled by the frontend to determine if the coordinator
/// is running and what searches are active.
#[tokio::test]
async fn get_status_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/status").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("active").is_some());
}

/// Verifies the /api/fleet endpoint returns 200 with empty worker list.
///
/// Exercises: GET /api/fleet, fleet status aggregation.
///
/// With a freshly truncated database, the fleet should show zero workers.
/// The response includes both the worker list and a total_workers count.
#[tokio::test]
async fn get_fleet_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/fleet").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("workers").is_some());
    assert_eq!(json["total_workers"], 0);
}

/// Verifies the /api/searches endpoint returns 200 with a searches array.
///
/// Exercises: GET /api/searches, active search listing.
///
/// Returns the list of in-progress searches managed by the coordinator.
#[tokio::test]
async fn get_searches_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/searches").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("searches").is_some());
}

/// Verifies the /api/events endpoint returns 200 with an events array.
///
/// Exercises: GET /api/events, system event stream.
///
/// Returns recent system events (prime discoveries, worker joins/leaves, etc.).
#[tokio::test]
async fn get_events_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/events").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("events").is_some());
}

/// Verifies the /api/notifications endpoint returns 200 with a notifications array.
///
/// Exercises: GET /api/notifications, prime discovery notification feed.
///
/// Returns notifications for the frontend bell icon / toast system.
#[tokio::test]
async fn get_notifications_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/notifications").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("notifications").is_some());
}

/// Verifies the /api/docs endpoint returns 200 with a docs listing.
///
/// Exercises: GET /api/docs, documentation index.
///
/// Returns the list of available documentation pages served from markdown files.
#[tokio::test]
async fn get_docs_returns_200() {
    require_db!();
    let (status, json) = get(app().await, "/api/docs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("docs").is_some());
}

/// Verifies the volunteer worker latest-release endpoint returns channel metadata.
///
/// Exercises: GET /api/volunteer/worker/latest, release channel lookup.
///
/// Queries the "stable" channel and verifies the response contains the expected
/// fields (channel, version, artifacts array). This endpoint is polled by
/// volunteer workers to check for auto-updates.
#[tokio::test]
async fn get_volunteer_worker_latest_returns_channel_release() {
    require_db!();
    let (status, json) = get(app().await, "/api/volunteer/worker/latest?channel=stable").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["channel"], "stable");
    assert!(json["version"].is_string());
    assert!(json["artifacts"].is_array());
}

/// Tests the full release lifecycle: publish releases, assign to channels, query latest.
///
/// Exercises: POST /api/releases/worker (publish), POST /api/releases/rollout
/// (channel assignment), GET /api/volunteer/worker/latest (query),
/// GET /api/releases/events, GET /api/releases/health.
///
/// This is an end-to-end test of the release control plane:
/// 1. Publish two releases (9.9.8-test, 9.9.9-test) with Linux x86_64 artifacts
/// 2. Roll out 9.9.8-test at 100% to stable channel
/// 3. Roll out 9.9.9-test at 0% (staged, not yet active)
/// 4. Verify /latest returns 9.9.8-test (the fully rolled out version)
/// 5. Promote 9.9.9-test to 100%
/// 6. Verify /latest now returns 9.9.9-test
/// 7. Check release events and health endpoints return valid data
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
        router.clone(),
        "/api/volunteer/worker/latest?channel=stable&worker_id=abc",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["version"], "9.9.9-test");

    let (status, json) = get(
        router.clone(),
        "/api/releases/events?channel=stable&limit=10",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["events"].is_array());

    let (status, json) = get(router, "/api/releases/health?active_hours=24").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["adoption"].is_array());
    assert!(json["channels"].is_array());
}

/// Tests that publishing a release with non-array artifacts returns 400.
///
/// Exercises: POST /api/releases/worker input validation.
///
/// The artifacts field must be a JSON array of platform-specific binaries.
/// Passing a plain object should be rejected with a descriptive error message.
#[tokio::test]
async fn releases_upsert_rejects_non_array_artifacts() {
    require_db!();
    let router = app().await;

    let (status, json) = post_json(
        router,
        "/api/releases/worker",
        serde_json::json!({
            "version": "bad-artifacts-test",
            "artifacts": { "os": "linux" }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "artifacts must be a JSON array");
}

// == Worker API ================================================================
// Tests for the internal worker-to-coordinator API: registration, heartbeat,
// prime submission, and deregistration. These endpoints are called by darkreach
// worker processes, not by browsers.
// ==============================================================================

/// Tests the full worker lifecycle: register -> heartbeat -> verify fleet -> deregister.
///
/// Exercises: POST /api/worker/register, POST /api/worker/heartbeat,
/// GET /api/fleet, POST /api/worker/deregister.
///
/// Registers a worker with 8 cores running a factorial search, sends a heartbeat
/// reporting progress (42 tested, 3 found), verifies the worker appears in the
/// fleet listing, then deregisters. Each step verifies the response is 200 OK
/// with `{"ok": true}`.
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

/// Tests prime submission via the worker API.
///
/// Exercises: POST /api/worker/prime, `primes` table INSERT.
///
/// Workers call this endpoint when they discover a prime during their search.
/// The prime is stored in the database with form, expression, digits, search
/// parameters, and proof method.
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

// == Search Job API ============================================================
// Tests for the search job management endpoints: listing, creation with block
// generation, detail retrieval, input validation, and cancellation.
// ==============================================================================

/// Tests that the search jobs list is empty in a clean database.
///
/// Exercises: GET /api/search_jobs, empty state handling.
///
/// Verifies the endpoint returns an empty array rather than an error when
/// no search jobs exist.
#[tokio::test]
async fn get_search_jobs_empty() {
    require_db!();
    let (status, json) = get(app().await, "/api/search_jobs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["search_jobs"].as_array().unwrap().len(), 0);
}

/// Tests search job creation with automatic block generation.
///
/// Exercises: POST /api/search_jobs (201 Created), block count calculation,
/// GET /api/search_jobs (list verification).
///
/// Creates a factorial search job spanning [1, 500] with block_size 100.
/// Verifies the response includes the job ID and correct block count (5).
/// Then confirms the job appears in the search jobs listing.
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

/// Tests that creating a search job with an invalid range returns 400.
///
/// Exercises: POST /api/search_jobs input validation (range_start >= range_end).
///
/// Submitting range_start=500, range_end=100 should be rejected since it
/// represents an empty or inverted search range.
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

/// Tests retrieving a specific search job's detail view.
///
/// Exercises: GET /api/search_jobs/{id}, job detail with block information.
///
/// Creates a kbn search job, then fetches its detail page. Verifies the
/// response includes the job metadata (search_type) and a blocks section.
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

/// Tests cancelling a running search job via the API.
///
/// Exercises: POST /api/search_jobs/{id}/cancel, status transition to "cancelled".
///
/// Creates a job, cancels it via the API endpoint, then verifies the job's
/// status has changed to "cancelled" in the detail view.
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

// == Agent API =================================================================
// Tests for the agent management REST endpoints: task creation, retrieval,
// event listing, and budget information.
// ==============================================================================

/// Tests agent task creation and retrieval via the REST API.
///
/// Exercises: POST /api/agents/tasks (201 Created), GET /api/agents/tasks/{id},
/// GET /api/agents/tasks (list).
///
/// Creates a high-priority task via the API, verifies the response contains
/// correct fields and "pending" status, then retrieves it by ID and via the
/// task listing endpoint.
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

/// Tests the agent events endpoint returns valid data.
///
/// Exercises: GET /api/agents/events, event listing.
///
/// Verifies the endpoint returns 200 OK with an "events" key, even when
/// no events have been generated yet.
#[tokio::test]
async fn get_agent_events() {
    require_db!();
    let (status, json) = get(app().await, "/api/agents/events").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("events").is_some());
}

/// Tests the agent budgets endpoint returns the seeded budget periods.
///
/// Exercises: GET /api/agents/budgets, `agent_budgets` table SELECT.
///
/// The test database is seeded with 3 budget periods (daily, weekly, monthly).
/// Verifies all 3 are returned. These budgets control maximum agent spending
/// per time window.
#[tokio::test]
async fn get_agent_budgets() {
    require_db!();
    let (status, json) = get(app().await, "/api/agents/budgets").await;
    assert_eq!(status, StatusCode::OK);
    // Should have the 3 seeded budgets (daily, weekly, monthly)
    assert_eq!(json["budgets"].as_array().unwrap().len(), 3);
}

// == Middleware Tests ===========================================================
// Tests verifying cross-cutting middleware behavior: CORS headers and request
// body size limits. These protect the API from cross-origin attacks and
// denial-of-service via oversized payloads.
// ==============================================================================

/// Tests that CORS headers are included in responses to cross-origin requests.
///
/// Exercises: CORS middleware, `access-control-allow-origin` response header.
///
/// Sends a request with an `Origin` header and verifies the response includes
/// the `access-control-allow-origin` header. Without this, the frontend at
/// `app.darkreach.ai` would be blocked from calling the API at `api.darkreach.ai`.
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

/// Tests that oversized request bodies are rejected with 413 Payload Too Large.
///
/// Exercises: body size limit middleware (1MB limit), HTTP 413 response.
///
/// Sends a 2MB payload to the worker registration endpoint. The body limit
/// middleware should reject this before it reaches the handler. This prevents
/// memory exhaustion from malicious or accidental oversized requests.
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

// == Error Cases ===============================================================
// Tests for proper error responses: 404 for missing resources, 400 for
// invalid input, path traversal rejection.
// ==============================================================================

/// Tests that requesting a non-existent search job returns 404.
///
/// Exercises: GET /api/search_jobs/{id} with invalid ID, 404 Not Found response.
///
/// Uses a very high ID (99999) that will not exist in the clean test database.
#[tokio::test]
async fn search_job_not_found() {
    require_db!();
    let (status, json) = get(app().await, "/api/search_jobs/99999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(json.get("error").is_some());
}

/// Tests that requesting a non-existent documentation slug returns 404.
///
/// Exercises: GET /api/docs/{slug} with invalid slug, 404 Not Found response.
#[tokio::test]
async fn doc_not_found() {
    require_db!();
    let (status, json) = get(app().await, "/api/docs/nonexistent-slug").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(json.get("error").is_some());
}

/// Tests that path traversal attempts in doc slugs are rejected.
///
/// Exercises: GET /api/docs/{slug} with `../` traversal, 400 Bad Request.
///
/// This is a security test (also covered more extensively in security_tests.rs).
/// The slug validator should reject any slug containing path traversal sequences
/// to prevent reading arbitrary files from the filesystem.
/// See also: OWASP Path Traversal (CWE-22).
#[tokio::test]
async fn doc_slug_rejects_path_traversal() {
    require_db!();
    let (status, _) = get(app().await, "/api/docs/../../../etc/passwd").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// == Operator API (v1) =========================================================
// Tests for the volunteer compute operator API: registration with validation,
// work claiming, and the public leaderboard. These endpoints are called by
// external operators who contribute compute resources.
// ==============================================================================

/// Tests operator registration with input validation.
///
/// Exercises: POST /api/v1/register (201 Created, 409 Conflict, 400 Bad Request).
///
/// Covers four scenarios:
/// 1. Successful registration: returns 201 with username and generated api_key
/// 2. Duplicate username: returns 409 Conflict
/// 3. Username too short (<3 chars): returns 400 with validation error
/// 4. Invalid email format: returns 400 with validation error
///
/// The username/email validation prevents garbage data and the duplicate check
/// prevents account takeover by re-registering existing usernames.
#[tokio::test]
async fn operator_register_endpoint() {
    require_db!();
    let router = app().await;

    // Successful registration
    let (status, json) = post_json(
        router.clone(),
        "/api/v1/register",
        serde_json::json!({
            "username": "api_test_user",
            "email": "apiuser@example.com"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(json.get("api_key").is_some(), "Response should contain api_key");
    assert_eq!(json["username"], "api_test_user");

    // Duplicate registration returns 409 Conflict
    let (status, json) = post_json(
        router.clone(),
        "/api/v1/register",
        serde_json::json!({
            "username": "api_test_user",
            "email": "different@example.com"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(json.get("error").is_some());

    // Invalid username (too short) returns 400
    let (status, json) = post_json(
        router.clone(),
        "/api/v1/register",
        serde_json::json!({
            "username": "ab",
            "email": "short@example.com"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"].as_str().unwrap().contains("3-32 characters"));

    // Invalid email returns 400
    let (status, json) = post_json(
        router.clone(),
        "/api/v1/register",
        serde_json::json!({
            "username": "validname",
            "email": "not-an-email"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"].as_str().unwrap().contains("email"));
}

/// Tests that work claiming returns 204 No Content when no jobs are available.
///
/// Exercises: GET /api/v1/work with Bearer token authentication, 204 No Content.
///
/// Registers an operator, then requests work from an empty job queue. The
/// endpoint should return 204 (not 404 or 500) to indicate "no work available
/// right now, try again later". The worker polls this endpoint periodically.
#[tokio::test]
async fn operator_work_claim_returns_204_when_empty() {
    require_db!();
    let router = app().await;

    // Register an operator to get an API key
    let (status, reg_json) = post_json(
        router.clone(),
        "/api/v1/register",
        serde_json::json!({
            "username": "empty_worker",
            "email": "empty@example.com"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let api_key = reg_json["api_key"].as_str().unwrap();

    // Request work with no search jobs available -- should return 204 No Content
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/work?cores=4&ram_gb=16")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::NO_CONTENT,
        "No work available should return 204"
    );
}

/// Tests the public leaderboard endpoint with operator registration.
///
/// Exercises: GET /api/v1/leaderboard (no authentication required),
/// leaderboard entry structure validation.
///
/// First verifies the leaderboard is empty in a clean database, then registers
/// two operators and verifies they appear in the leaderboard with the expected
/// fields (rank, username, credit, primes_found, worker_count). The leaderboard
/// is a public endpoint that does not require authentication.
#[tokio::test]
async fn operator_leaderboard_endpoint() {
    require_db!();
    let router = app().await;

    // Leaderboard should be accessible without authentication
    let (status, json) = get(router.clone(), "/api/v1/leaderboard").await;
    assert_eq!(status, StatusCode::OK);
    // Should be an array (empty since no operators registered)
    assert!(json.is_array(), "Leaderboard should return a JSON array");
    assert_eq!(json.as_array().unwrap().len(), 0);

    // Register some operators and give them credit via DB directly
    let (_, reg1) = post_json(
        router.clone(),
        "/api/v1/register",
        serde_json::json!({
            "username": "leader_one",
            "email": "leader1@example.com"
        }),
    )
    .await;
    let (_, reg2) = post_json(
        router.clone(),
        "/api/v1/register",
        serde_json::json!({
            "username": "leader_two",
            "email": "leader2@example.com"
        }),
    )
    .await;
    assert!(reg1.get("api_key").is_some());
    assert!(reg2.get("api_key").is_some());

    // Leaderboard should now show 2 entries
    let (status, json) = get(router.clone(), "/api/v1/leaderboard").await;
    assert_eq!(status, StatusCode::OK);
    let entries = json.as_array().unwrap();
    assert_eq!(entries.len(), 2);

    // Each entry should have expected fields
    for entry in entries {
        assert!(entry.get("rank").is_some());
        assert!(entry.get("username").is_some());
        assert!(entry.get("credit").is_some());
        assert!(entry.get("primes_found").is_some());
        assert!(entry.get("worker_count").is_some());
    }
}
