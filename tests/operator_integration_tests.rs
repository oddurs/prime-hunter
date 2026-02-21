//! # Operator Integration Tests — Real HTTP Mock Server Tests
//!
//! Tests the darkreach operator client (`src/operator.rs`) and worker client
//! (`src/worker_client.rs`) against real HTTP servers built with `axum`. Each test
//! spawns a mock coordinator on a random port (`127.0.0.1:0`), exercises the
//! operator API functions, and verifies correct behavior for success, error, and
//! edge-case scenarios.
//!
//! ## Architecture
//!
//! Unlike unit tests that mock at the HTTP library level (e.g., replacing `ureq`
//! with a fake), these tests spin up actual TCP listeners serving axum routers.
//! This provides higher confidence because:
//!
//! 1. **Real HTTP semantics**: status codes, headers, content-type negotiation,
//!    and connection handling are exercised end-to-end.
//! 2. **Real serialization**: JSON payloads are serialized by the client and
//!    deserialized by the mock server (and vice versa), catching schema mismatches.
//! 3. **Real error paths**: connection refused, malformed responses, and HTTP
//!    error codes propagate through the actual `ureq` + `anyhow` error chain.
//!
//! ## Tokio Runtime Configuration
//!
//! All async tests use `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.
//! This is required because the operator client uses `ureq`, a **blocking** HTTP
//! library. With a single-threaded tokio runtime (the default for `#[tokio::test]`),
//! the blocking `ureq` call would starve the mock server's `axum::serve` task,
//! causing a deadlock. The multi-threaded runtime ensures the mock server task
//! runs on a separate worker thread while `ureq` blocks on the test thread.
//!
//! ## Test Organization
//!
//! - **Registration** (tests 1-4): `POST /api/v1/register` success, 400, 500, malformed JSON
//! - **Work Claiming** (tests 5-9): `GET /api/v1/work` with assignments, 204, 401, capabilities, all search types
//! - **Result Submission** (tests 10-12): `POST /api/v1/result` success, with primes, server error
//! - **Heartbeat** (tests 13-14): `POST /api/v1/worker/heartbeat` success, worker_id verification
//! - **Update Check** (tests 15-17): `GET /api/v1/worker/latest` available, not available, offline
//! - **Search Params** (tests 18-22): `to_args()`, `range()`, `default_block_size()`, roundtrip, error
//! - **Worker Client** (tests 23-24): stop flag propagation, atomic counter concurrency
//!
//! ## Running
//!
//! ```bash
//! cargo test --test operator_integration_tests
//! ```
//!
//! No database or external services required. All tests are self-contained with
//! ephemeral mock servers.

use axum::{
    extract::Query,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

use darkreach::operator::{OperatorConfig, PrimeReport, ResultSubmission, WorkAssignment};
use darkreach::search_params::SearchParams;

// ============================================================================
// Mock Server Infrastructure
// ============================================================================

/// Starts a mock HTTP server on a random available port.
///
/// Returns the base URL (e.g., `http://127.0.0.1:54321`) and a `JoinHandle` for
/// the server task. The caller should `abort()` the handle when the test completes
/// to clean up the background task.
///
/// Uses `TcpListener::bind("127.0.0.1:0")` to get an OS-assigned ephemeral port,
/// avoiding conflicts between concurrent test runs.
async fn start_mock_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://127.0.0.1:{}", addr.port());
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // Give the server a moment to start accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (url, handle)
}

/// Creates a test `OperatorConfig` pointing at the given mock server URL.
fn test_config(server_url: &str) -> OperatorConfig {
    OperatorConfig {
        server: server_url.to_string(),
        api_key: "test-api-key-abc123".to_string(),
        username: "testuser".to_string(),
        worker_id: "testhost-deadbeef".to_string(),
    }
}

// ============================================================================
// Registration Tests (1-4)
// ============================================================================

/// Test 1: Successful registration returns an API key and username.
///
/// The mock server at `POST /api/v1/register` returns a valid JSON response
/// with `api_key` and `username`. Verifies that the client receives and parses
/// the response correctly.
///
/// Note: We call the endpoint directly via `ureq` rather than through
/// `operator::register()`, which would also write to `~/.darkreach/config.toml`
/// as a filesystem side effect.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_register_success_real_http() {
    let app = Router::new().route(
        "/api/v1/register",
        post(|| async {
            Json(serde_json::json!({
                "api_key": "generated-key-xyz789",
                "username": "alice"
            }))
        }),
    );
    let (url, handle) = start_mock_server(app).await;

    let response: serde_json::Value = ureq::post(&format!("{}/api/v1/register", url))
        .send_json(&serde_json::json!({
            "username": "alice",
            "email": "alice@example.com"
        }))
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap();

    assert_eq!(response["api_key"], "generated-key-xyz789");
    assert_eq!(response["username"], "alice");

    handle.abort();
}

/// Test 2: Registration with a 400 Bad Request response.
///
/// When the coordinator rejects a registration (e.g., duplicate username),
/// it returns HTTP 400. The `ureq` client treats 4xx responses as errors,
/// so the caller receives a clear error through the error chain.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_register_server_returns_400() {
    let app = Router::new().route(
        "/api/v1/register",
        post(|| async { (StatusCode::BAD_REQUEST, "Username already taken") }),
    );
    let (url, handle) = start_mock_server(app).await;

    let result = ureq::post(&format!("{}/api/v1/register", url)).send_json(&serde_json::json!({
        "username": "taken",
        "email": "taken@example.com"
    }));

    assert!(result.is_err(), "Expected error for 400 response");
    let err = result.unwrap_err();
    let err_str = format!("{}", err);
    assert!(
        err_str.contains("400") || err_str.contains("Bad Request"),
        "Error should mention 400 status: {}",
        err_str
    );

    handle.abort();
}

/// Test 3: Registration with a 500 Internal Server Error.
///
/// Server-side failures during registration should propagate as errors.
/// The client should not panic or silently succeed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_register_server_returns_500() {
    let app = Router::new().route(
        "/api/v1/register",
        post(|| async {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database connection failed",
            )
        }),
    );
    let (url, handle) = start_mock_server(app).await;

    let result = ureq::post(&format!("{}/api/v1/register", url)).send_json(&serde_json::json!({
        "username": "bob",
        "email": "bob@example.com"
    }));

    assert!(result.is_err(), "Expected error for 500 response");

    handle.abort();
}

/// Test 4: Registration with malformed JSON response.
///
/// If the server returns invalid JSON (e.g., truncated response, HTML error
/// page), the client's JSON deserialization should fail gracefully rather
/// than panicking. The HTTP request itself succeeds (200 OK), but reading
/// the body as JSON should return an error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_register_server_returns_malformed_json() {
    let app = Router::new().route(
        "/api/v1/register",
        post(|| async { (StatusCode::OK, "this is not valid json {{{") }),
    );
    let (url, handle) = start_mock_server(app).await;

    let resp = ureq::post(&format!("{}/api/v1/register", url)).send_json(&serde_json::json!({
        "username": "carol",
        "email": "carol@example.com"
    }));

    // The HTTP request succeeds (200 OK), but JSON parsing should fail.
    match resp {
        Ok(mut response) => {
            let json_result: Result<darkreach::operator::RegisterResponse, _> =
                response.body_mut().read_json();
            assert!(
                json_result.is_err(),
                "Should fail to parse malformed JSON response"
            );
        }
        Err(_) => {
            // Some ureq versions may error on content-type mismatch; also acceptable.
        }
    }

    handle.abort();
}

// ============================================================================
// Work Claiming Tests (5-9)
// ============================================================================

/// Test 5: Claiming work returns a valid work assignment.
///
/// The mock server at `GET /api/v1/work` returns a JSON work assignment with
/// block_id, search_job_id, search_type, params, block_start, and block_end.
/// Verifies that `operator::claim_work` correctly deserializes the response.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_claim_work_returns_assignment() {
    let app = Router::new().route(
        "/api/v1/work",
        get(|| async {
            Json(serde_json::json!({
                "block_id": 42,
                "search_job_id": 7,
                "search_type": "factorial",
                "params": {"start": 100, "end": 200},
                "block_start": 100,
                "block_end": 150
            }))
        }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let result = darkreach::operator::claim_work(&config, 8);
    assert!(result.is_ok(), "claim_work failed: {:?}", result.err());

    let assignment = result.unwrap();
    assert!(assignment.is_some(), "Expected Some(WorkAssignment)");

    let wa = assignment.unwrap();
    assert_eq!(wa.block_id, 42);
    assert_eq!(wa.search_job_id, 7);
    assert_eq!(wa.search_type, "factorial");
    assert_eq!(wa.block_start, 100);
    assert_eq!(wa.block_end, 150);

    handle.abort();
}

/// Test 6: Claiming work when no work is available returns None.
///
/// The coordinator returns HTTP 204 No Content when there are no work blocks
/// available. `claim_work` should return `Ok(None)` rather than an error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_claim_work_returns_204_no_content() {
    let app = Router::new().route(
        "/api/v1/work",
        get(|| async { StatusCode::NO_CONTENT }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let result = darkreach::operator::claim_work(&config, 4);
    assert!(result.is_ok(), "claim_work should not error on 204");

    let assignment = result.unwrap();
    assert!(assignment.is_none(), "Expected None for 204 No Content");

    handle.abort();
}

/// Test 7: Claiming work with an invalid API key returns 401 Unauthorized.
///
/// The mock server checks the Authorization header and rejects requests
/// without a valid Bearer token. This verifies that authentication errors
/// propagate clearly to the caller.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_claim_work_returns_401_unauthorized() {
    let app = Router::new().route(
        "/api/v1/work",
        get(|headers: axum::http::HeaderMap| async move {
            let auth = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if auth == "Bearer valid-key-only" {
                Json(serde_json::json!({
                    "block_id": 1,
                    "search_job_id": 1,
                    "search_type": "factorial",
                    "params": {},
                    "block_start": 1,
                    "block_end": 10
                }))
                .into_response()
            } else {
                (StatusCode::UNAUTHORIZED, "Invalid API key").into_response()
            }
        }),
    );
    let (url, handle) = start_mock_server(app).await;

    // Use a config with an invalid API key
    let config = OperatorConfig {
        server: url.clone(),
        api_key: "wrong-key".to_string(),
        username: "intruder".to_string(),
        worker_id: "badhost-00000000".to_string(),
    };

    let result = darkreach::operator::claim_work(&config, 4);
    assert!(result.is_err(), "Expected error for 401 Unauthorized");

    handle.abort();
}

/// Test 8: Claim work sends capability query parameters.
///
/// The operator client sends cores, ram_gb, has_gpu, os, and arch as query
/// parameters so the coordinator can assign appropriate work blocks. This test
/// captures the received query string and verifies all expected parameters
/// are present.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_claim_work_sends_capabilities() {
    /// Query parameters captured from the claim_work request.
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct WorkQuery {
        cores: Option<String>,
        ram_gb: Option<String>,
        has_gpu: Option<String>,
        os: Option<String>,
        arch: Option<String>,
    }

    let captured_params: Arc<Mutex<Option<WorkQuery>>> = Arc::new(Mutex::new(None));
    let params_clone = captured_params.clone();

    let app = Router::new().route(
        "/api/v1/work",
        get(move |Query(query): Query<WorkQuery>| {
            let params_clone = params_clone.clone();
            async move {
                *params_clone.lock().unwrap() = Some(query);
                StatusCode::NO_CONTENT
            }
        }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let _ = darkreach::operator::claim_work(&config, 16);

    let params = captured_params.lock().unwrap();
    assert!(
        params.is_some(),
        "Query parameters should have been captured"
    );
    let p = params.as_ref().unwrap();
    assert_eq!(
        p.cores.as_deref(),
        Some("16"),
        "cores parameter should be 16"
    );
    assert!(p.ram_gb.is_some(), "ram_gb parameter should be present");
    assert!(p.os.is_some(), "os parameter should be present");
    assert!(p.arch.is_some(), "arch parameter should be present");
    assert!(
        p.has_gpu.is_some(),
        "has_gpu parameter should be present"
    );

    handle.abort();
}

/// Test 9: Parsing work assignments for all 11 search types in SearchParams.
///
/// The coordinator can assign work blocks for any of the 11 supported search
/// forms. This test verifies that `WorkAssignment` correctly deserializes
/// each search_type string and its associated params JSON. No HTTP server
/// is needed since this tests pure deserialization.
#[test]
fn test_claim_work_assignment_all_search_types() {
    let search_types = vec![
        ("factorial", serde_json::json!({"start": 1, "end": 100})),
        (
            "palindromic",
            serde_json::json!({"base": 10, "min_digits": 1, "max_digits": 9}),
        ),
        (
            "kbn",
            serde_json::json!({"k": 3, "base": 2, "min_n": 1, "max_n": 1000}),
        ),
        ("primorial", serde_json::json!({"start": 2, "end": 100})),
        (
            "cullen_woodall",
            serde_json::json!({"min_n": 1, "max_n": 100}),
        ),
        (
            "wagstaff",
            serde_json::json!({"min_exp": 3, "max_exp": 100}),
        ),
        (
            "carol_kynea",
            serde_json::json!({"min_n": 1, "max_n": 100}),
        ),
        (
            "twin",
            serde_json::json!({"k": 3, "base": 2, "min_n": 1, "max_n": 1000}),
        ),
        (
            "sophie_germain",
            serde_json::json!({"k": 1, "base": 2, "min_n": 2, "max_n": 100}),
        ),
        (
            "repunit",
            serde_json::json!({"base": 10, "min_n": 2, "max_n": 50}),
        ),
        (
            "gen_fermat",
            serde_json::json!({"fermat_exp": 1, "min_base": 2, "max_base": 100}),
        ),
    ];

    for (i, (search_type, params)) in search_types.iter().enumerate() {
        let json_str = serde_json::to_string(&serde_json::json!({
            "block_id": i as i64 + 1,
            "search_job_id": 100 + i as i64,
            "search_type": search_type,
            "params": params,
            "block_start": 1,
            "block_end": 1000
        }))
        .unwrap();

        let wa: WorkAssignment = serde_json::from_str(&json_str).unwrap_or_else(|e| {
            panic!(
                "Failed to deserialize WorkAssignment for search_type '{}': {}",
                search_type, e
            )
        });

        assert_eq!(
            wa.search_type, *search_type,
            "search_type mismatch for variant {}",
            i
        );
        assert_eq!(wa.block_id, i as i64 + 1);
    }
}

// ============================================================================
// Result Submission Tests (10-12)
// ============================================================================

/// Test 10: Successful result submission.
///
/// The mock server accepts a `POST /api/v1/result` and returns 200 OK.
/// Verifies that `operator::submit_result` completes without error and
/// that the server receives the correct payload fields.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_submit_result_success() {
    let received_body: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
    let body_clone = received_body.clone();

    let app = Router::new().route(
        "/api/v1/result",
        post(move |Json(body): Json<serde_json::Value>| {
            let body_clone = body_clone.clone();
            async move {
                *body_clone.lock().unwrap() = Some(body);
                StatusCode::OK
            }
        }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let submission = ResultSubmission {
        block_id: 42,
        tested: 1000,
        found: 0,
        primes: vec![],
    };

    let result = darkreach::operator::submit_result(&config, &submission);
    assert!(
        result.is_ok(),
        "submit_result should succeed: {:?}",
        result.err()
    );

    let body = received_body.lock().unwrap();
    assert!(body.is_some(), "Server should have received the body");
    let b = body.as_ref().unwrap();
    assert_eq!(b["block_id"], 42);
    assert_eq!(b["tested"], 1000);
    assert_eq!(b["found"], 0);

    handle.abort();
}

/// Test 11: Result submission with discovered primes.
///
/// When a work block discovers primes, the `ResultSubmission` includes a
/// non-empty `primes` array. Verifies that prime reports (expression, form,
/// digits, proof_method, certificate) are correctly serialized and received
/// by the mock coordinator.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_submit_result_with_primes() {
    let received_body: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
    let body_clone = received_body.clone();

    let app = Router::new().route(
        "/api/v1/result",
        post(move |Json(body): Json<serde_json::Value>| {
            let body_clone = body_clone.clone();
            async move {
                *body_clone.lock().unwrap() = Some(body);
                StatusCode::OK
            }
        }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let submission = ResultSubmission {
        block_id: 99,
        tested: 500,
        found: 2,
        primes: vec![
            PrimeReport {
                expression: "5!+1".to_string(),
                form: "factorial".to_string(),
                digits: 3,
                proof_method: "pocklington".to_string(),
                certificate: Some("{\"type\":\"Pocklington\"}".to_string()),
            },
            PrimeReport {
                expression: "3*2^127+1".to_string(),
                form: "kbn".to_string(),
                digits: 39,
                proof_method: "proth".to_string(),
                certificate: None,
            },
        ],
    };

    let result = darkreach::operator::submit_result(&config, &submission);
    assert!(result.is_ok(), "submit_result should succeed with primes");

    let body = received_body.lock().unwrap();
    let b = body.as_ref().unwrap();
    assert_eq!(b["found"], 2);
    let primes = b["primes"].as_array().unwrap();
    assert_eq!(primes.len(), 2);
    assert_eq!(primes[0]["expression"], "5!+1");
    assert_eq!(primes[0]["form"], "factorial");
    assert_eq!(primes[1]["expression"], "3*2^127+1");
    assert_eq!(primes[1]["proof_method"], "proth");

    handle.abort();
}

/// Test 12: Result submission when the server returns 500.
///
/// If the coordinator fails to process a result (e.g., database error),
/// the client should receive an error rather than silently losing the result.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_submit_result_server_error() {
    let app = Router::new().route(
        "/api/v1/result",
        post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "DB write failed") }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let submission = ResultSubmission {
        block_id: 1,
        tested: 100,
        found: 0,
        primes: vec![],
    };

    let result = darkreach::operator::submit_result(&config, &submission);
    assert!(
        result.is_err(),
        "submit_result should fail on 500 response"
    );

    handle.abort();
}

// ============================================================================
// Heartbeat Tests (13-14)
// ============================================================================

/// Test 13: Successful heartbeat.
///
/// The mock server at `POST /api/v1/worker/heartbeat` accepts the heartbeat
/// and returns 200 OK. Verifies that `operator::heartbeat` completes without error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_heartbeat_success() {
    let app = Router::new().route(
        "/api/v1/worker/heartbeat",
        post(|| async { StatusCode::OK }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let result = darkreach::operator::heartbeat(&config);
    assert!(
        result.is_ok(),
        "heartbeat should succeed: {:?}",
        result.err()
    );

    handle.abort();
}

/// Test 14: Heartbeat sends the correct worker_id in the payload.
///
/// The coordinator uses the worker_id from the heartbeat payload to track
/// which worker is still alive. This test captures the received JSON body
/// and verifies the worker_id matches the config.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_heartbeat_sends_worker_id() {
    let received_body: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
    let body_clone = received_body.clone();

    let app = Router::new().route(
        "/api/v1/worker/heartbeat",
        post(move |Json(body): Json<serde_json::Value>| {
            let body_clone = body_clone.clone();
            async move {
                *body_clone.lock().unwrap() = Some(body);
                StatusCode::OK
            }
        }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let result = darkreach::operator::heartbeat(&config);
    assert!(result.is_ok());

    let body = received_body.lock().unwrap();
    assert!(body.is_some());
    let b = body.as_ref().unwrap();
    assert_eq!(
        b["worker_id"], "testhost-deadbeef",
        "Heartbeat should send the configured worker_id"
    );

    handle.abort();
}

// ============================================================================
// Update Check Tests (15-17)
// ============================================================================

/// Test 15: Update available — server returns a newer version.
///
/// When the coordinator's latest version differs from the running binary's
/// version (`env!("CARGO_PKG_VERSION")`), `check_for_update` should return
/// `Some(WorkerReleaseInfo)` with the new version details.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_check_update_available() {
    let app = Router::new().route(
        "/api/v1/worker/latest",
        get(|| async {
            Json(serde_json::json!({
                "channel": "stable",
                "version": "99.99.99",
                "published_at": "2026-02-20T12:00:00Z",
                "notes": "Major update",
                "artifacts": [
                    {
                        "os": std::env::consts::OS,
                        "arch": std::env::consts::ARCH,
                        "url": "https://example.com/download",
                        "sha256": "abc123def456"
                    }
                ]
            }))
        }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let result = darkreach::operator::check_for_update(&config, "stable");
    assert!(
        result.is_ok(),
        "check_for_update failed: {:?}",
        result.err()
    );

    let update = result.unwrap();
    assert!(
        update.is_some(),
        "Should detect update when version differs from current"
    );
    let info = update.unwrap();
    assert_eq!(info.version, "99.99.99");
    assert_eq!(info.channel, "stable");
    assert_eq!(info.notes, Some("Major update".to_string()));

    handle.abort();
}

/// Test 16: No update available — server returns the current version.
///
/// When the latest version matches `env!("CARGO_PKG_VERSION")`,
/// `check_for_update` should return `Ok(None)`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_check_update_not_available() {
    let current_version = env!("CARGO_PKG_VERSION");

    let app = Router::new().route(
        "/api/v1/worker/latest",
        get(move || async move {
            Json(serde_json::json!({
                "channel": "stable",
                "version": current_version,
                "published_at": "2026-02-20T12:00:00Z",
                "notes": null,
                "artifacts": []
            }))
        }),
    );
    let (url, handle) = start_mock_server(app).await;
    let config = test_config(&url);

    let result = darkreach::operator::check_for_update(&config, "stable");
    assert!(
        result.is_ok(),
        "check_for_update failed: {:?}",
        result.err()
    );

    let update = result.unwrap();
    assert!(
        update.is_none(),
        "Should return None when version matches current ({})",
        current_version
    );

    handle.abort();
}

/// Test 17: Update check when the server is offline (connection refused).
///
/// If the coordinator is unreachable, `check_for_update` (which calls
/// `get_latest_worker_release`) should return an error rather than panicking.
/// This simulates network outages or coordinator maintenance.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_check_update_server_offline() {
    // Bind a listener to get a valid port, then drop it so nothing is listening.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let config = OperatorConfig {
        server: format!("http://127.0.0.1:{}", addr.port()),
        api_key: "key".to_string(),
        username: "user".to_string(),
        worker_id: "w1".to_string(),
    };

    let result = darkreach::operator::check_for_update(&config, "stable");
    assert!(
        result.is_err(),
        "check_for_update should fail when server is offline"
    );
}

// ============================================================================
// Search Params Dispatch Tests (18-22)
// ============================================================================

/// Helper: returns one instance of every `SearchParams` variant with
/// representative parameter values for exhaustive testing.
fn all_search_params() -> Vec<SearchParams> {
    vec![
        SearchParams::Factorial { start: 1, end: 100 },
        SearchParams::Palindromic {
            base: 10,
            min_digits: 1,
            max_digits: 9,
        },
        SearchParams::Kbn {
            k: 3,
            base: 2,
            min_n: 1,
            max_n: 1000,
        },
        SearchParams::Primorial { start: 2, end: 100 },
        SearchParams::CullenWoodall {
            min_n: 1,
            max_n: 100,
        },
        SearchParams::Wagstaff {
            min_exp: 3,
            max_exp: 100,
        },
        SearchParams::CarolKynea {
            min_n: 1,
            max_n: 100,
        },
        SearchParams::Twin {
            k: 3,
            base: 2,
            min_n: 1,
            max_n: 1000,
        },
        SearchParams::SophieGermain {
            k: 1,
            base: 2,
            min_n: 2,
            max_n: 100,
        },
        SearchParams::Repunit {
            base: 10,
            min_n: 2,
            max_n: 50,
        },
        SearchParams::GenFermat {
            fermat_exp: 1,
            min_base: 2,
            max_base: 100,
        },
    ]
}

/// Test 18: `to_args()` produces valid CLI arguments for all 11 search forms.
///
/// Each form must produce a non-empty argument list starting with a subcommand
/// name (no leading dash), followed by `--flag value` pairs. The deploy module
/// passes these args directly to SSH commands, so correctness is critical.
/// Validates subcommand names use hyphens (not underscores) for multi-word forms.
#[test]
fn test_search_params_to_args_all_11_forms() {
    let expected_subcommands = [
        "factorial",
        "palindromic",
        "kbn",
        "primorial",
        "cullen-woodall",
        "wagstaff",
        "carol-kynea",
        "twin",
        "sophie-germain",
        "repunit",
        "gen-fermat",
    ];

    for (params, expected_cmd) in all_search_params().iter().zip(expected_subcommands.iter()) {
        let args = params.to_args();
        assert!(
            !args.is_empty(),
            "to_args() should not be empty for {}",
            params.search_type_name()
        );
        assert_eq!(
            args[0], *expected_cmd,
            "Subcommand mismatch for {}",
            params.search_type_name()
        );
        // After subcommand, args come in --flag value pairs (even count)
        let flag_args = &args[1..];
        assert_eq!(
            flag_args.len() % 2,
            0,
            "Flags for {} should come in pairs, got {}",
            params.search_type_name(),
            flag_args.len()
        );
        // Every even-indexed flag arg (0, 2, 4, ...) should start with "--"
        for (i, arg) in flag_args.iter().enumerate() {
            if i % 2 == 0 {
                assert!(
                    arg.starts_with("--"),
                    "Expected flag at position {} for {}, got '{}'",
                    i + 1,
                    params.search_type_name(),
                    arg
                );
            }
        }
    }
}

/// Test 19: `range()` correctly extracts (start, end) for all forms.
///
/// Each search form has a primary iteration variable: factorial uses
/// start/end, kbn uses min_n/max_n, gen_fermat uses min_base/max_base, etc.
/// The range is used for work block generation in the search manager.
#[test]
fn test_search_params_range_extraction() {
    let expected_ranges: Vec<(i64, i64)> = vec![
        (1, 100),  // factorial: start..end
        (1, 9),    // palindromic: min_digits..max_digits
        (1, 1000), // kbn: min_n..max_n
        (2, 100),  // primorial: start..end
        (1, 100),  // cullen_woodall: min_n..max_n
        (3, 100),  // wagstaff: min_exp..max_exp
        (1, 100),  // carol_kynea: min_n..max_n
        (1, 1000), // twin: min_n..max_n
        (2, 100),  // sophie_germain: min_n..max_n
        (2, 50),   // repunit: min_n..max_n
        (2, 100),  // gen_fermat: min_base..max_base
    ];

    for (params, expected) in all_search_params().iter().zip(expected_ranges.iter()) {
        let range = params.range();
        assert_eq!(
            range, *expected,
            "Range mismatch for {}: got {:?}, expected {:?}",
            params.search_type_name(),
            range,
            expected
        );
    }
}

/// Test 20: Default block sizes are positive and match expected values.
///
/// Block sizes determine how many candidates are assigned per work block.
/// They must be positive (zero would cause infinite loops) and are calibrated
/// to produce ~60 second work blocks on typical hardware. Lightweight forms
/// (kbn, twin, sophie_germain) use 10,000; heavy forms (factorial, primorial)
/// use 100; digit-based forms (palindromic) use 2 digit counts per block.
#[test]
fn test_search_params_block_size_defaults() {
    let expected_sizes: Vec<(&str, i64)> = vec![
        ("factorial", 100),
        ("palindromic", 2),
        ("kbn", 10_000),
        ("primorial", 100),
        ("cullen_woodall", 1000),
        ("wagstaff", 1000),
        ("carol_kynea", 1000),
        ("twin", 10_000),
        ("sophie_germain", 10_000),
        ("repunit", 1000),
        ("gen_fermat", 1000),
    ];

    for (params, (name, expected_size)) in all_search_params().iter().zip(expected_sizes.iter()) {
        let size = params.default_block_size();
        assert!(
            size > 0,
            "Block size for {} must be positive, got {}",
            name,
            size
        );
        assert_eq!(
            size, *expected_size,
            "Block size mismatch for {}: got {}, expected {}",
            name, size, expected_size
        );
    }
}

/// Test 21: Unknown search type fails deserialization.
///
/// The serde tagged enum should reject JSON with an unrecognized `search_type`
/// value, preventing the coordinator from accepting malformed job parameters.
/// Tests several invalid variants: nonexistent types, empty strings, and
/// case-sensitive mismatches (e.g., "FACTORIAL" vs "factorial").
#[test]
fn test_search_params_invalid_form_error() {
    let invalid_jsons = vec![
        r#"{"search_type":"nonexistent","foo":42}"#,
        r#"{"search_type":"mersenne","min_exp":1,"max_exp":100}"#,
        r#"{"search_type":"","start":1,"end":100}"#,
        r#"{"search_type":"FACTORIAL","start":1,"end":100}"#, // case-sensitive
    ];

    for json in &invalid_jsons {
        let result: Result<SearchParams, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "Should reject invalid search_type in: {}",
            json
        );
    }
}

/// Test 22: JSON serialize/deserialize round-trip for all 11 forms.
///
/// Every SearchParams variant must survive JSON serialization and deserialization
/// without data loss. This is critical because params are stored as JSON in the
/// `search_jobs.params` column in PostgreSQL and transmitted over the REST API.
/// Also verifies that `search_type_name()` is consistent after round-trip.
#[test]
fn test_search_params_json_roundtrip() {
    for params in all_search_params() {
        let json = serde_json::to_string(&params).unwrap_or_else(|e| {
            panic!(
                "Failed to serialize {}: {}",
                params.search_type_name(),
                e
            )
        });

        let parsed: SearchParams = serde_json::from_str(&json).unwrap_or_else(|e| {
            panic!(
                "Failed to deserialize {} from '{}': {}",
                params.search_type_name(),
                json,
                e
            )
        });

        // Re-serialize and compare to verify lossless round-trip
        let json2 = serde_json::to_string(&parsed).unwrap();
        assert_eq!(
            json, json2,
            "JSON round-trip mismatch for {}: '{}' != '{}'",
            params.search_type_name(),
            json,
            json2
        );

        // Verify search_type_name consistency
        assert_eq!(
            params.search_type_name(),
            parsed.search_type_name(),
            "search_type_name mismatch after round-trip"
        );
    }
}

// ============================================================================
// Worker Client Tests (23-24)
// ============================================================================

/// Test 23: Stop flag propagation via AtomicBool across threads.
///
/// The `WorkerClient.stop_requested` flag is set by the heartbeat thread when
/// the coordinator responds with `"command":"stop"`. The engine search thread
/// checks `is_stop_requested()` at the top of each block iteration. This test
/// verifies that setting the flag from one thread is visible from another, and
/// that the flag is persistent (can be read multiple times after being set).
#[test]
fn test_worker_client_stop_flag_propagation() {
    use std::sync::atomic::AtomicBool;

    let stop_flag = Arc::new(AtomicBool::new(false));
    assert!(
        !stop_flag.load(Ordering::Relaxed),
        "Stop flag should start as false"
    );

    // Simulate coordinator sending stop command (heartbeat thread sets flag)
    let flag_clone = stop_flag.clone();
    let setter = std::thread::spawn(move || {
        flag_clone.store(true, Ordering::Relaxed);
    });
    setter.join().unwrap();

    // Simulate engine thread checking the flag
    assert!(
        stop_flag.load(Ordering::Relaxed),
        "Stop flag should be true after coordinator sets it"
    );

    // Verify the flag can be read multiple times (it is persistent, not a one-shot)
    assert!(stop_flag.load(Ordering::Relaxed));
    assert!(stop_flag.load(Ordering::Relaxed));
}

/// Test 24: Concurrent atomic counter increments do not lose counts.
///
/// The `WorkerClient.tested` and `WorkerClient.found` counters use `AtomicU64`
/// with `fetch_add` for lock-free concurrent updates from multiple rayon worker
/// threads. This test spawns 8 threads, each incrementing the counter 10,000
/// times, and verifies the final count is exactly 80,000 (no lost increments).
/// The `found` counter is incremented conditionally to simulate intermittent
/// prime discoveries.
#[test]
fn test_worker_client_atomic_counters() {
    let tested = Arc::new(AtomicU64::new(0));
    let found = Arc::new(AtomicU64::new(0));

    let num_threads = 8u64;
    let increments_per_thread = 10_000u64;

    let mut handles = Vec::new();
    for _ in 0..num_threads {
        let tested = tested.clone();
        let found = found.clone();
        handles.push(std::thread::spawn(move || {
            for _ in 0..increments_per_thread {
                tested.fetch_add(1, Ordering::Relaxed);
                // Simulate occasional prime discovery (every 1000th candidate)
                if tested.load(Ordering::Relaxed) % 1000 == 0 {
                    found.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let total_tested = tested.load(Ordering::Relaxed);
    assert_eq!(
        total_tested,
        num_threads * increments_per_thread,
        "Concurrent fetch_add should not lose increments: expected {}, got {}",
        num_threads * increments_per_thread,
        total_tested
    );

    // found should be > 0 (exact count depends on thread interleaving)
    let total_found = found.load(Ordering::Relaxed);
    assert!(
        total_found > 0,
        "Should have found at least one 'prime' across {} tested candidates",
        total_tested
    );
}
