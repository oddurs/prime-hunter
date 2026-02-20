//! Security-focused integration tests.
//!
//! Tests SQL injection prevention, request limits, CORS, path traversal,
//! and timeout enforcement at the API level.
//!
//! Requires TEST_DATABASE_URL to be set.
//! Run with: cargo test --test security_tests -- --test-threads=1

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

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

// ---------------------------------------------------------------------------
// SQL injection tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sql_injection_sort_column_sanitized() {
    require_db!();
    // Attempt SQL injection via sort_by parameter
    let injections = [
        "'; DROP TABLE primes; --",
        "1; DELETE FROM primes",
        "expression UNION SELECT * FROM pg_tables --",
        "id; UPDATE primes SET form='hacked'",
    ];

    for injection in &injections {
        let uri = format!("/api/primes?sort_by={}", urlencoding::encode(injection));
        let (status, json) = get(app().await, &uri).await;
        // Should succeed (injected value falls through to default "id")
        assert_eq!(
            status,
            StatusCode::OK,
            "Injection attempt should not crash: {}",
            injection
        );
        // Should return valid JSON array
        assert!(
            json.is_array(),
            "Response should be valid JSON array for: {}",
            injection
        );
    }
}

#[tokio::test]
async fn sql_injection_sort_dir_sanitized() {
    require_db!();
    let injections = [
        "DESC; DROP TABLE primes; --",
        "asc UNION SELECT 1,2,3,4,5",
        "'; --",
    ];

    for injection in &injections {
        let uri = format!("/api/primes?sort_dir={}", urlencoding::encode(injection));
        let (status, _) = get(app().await, &uri).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "Sort dir injection should not crash: {}",
            injection
        );
    }
}

#[tokio::test]
async fn sql_injection_search_param_escaped() {
    require_db!();
    // Attempt SQL injection via the search parameter (used in ILIKE with parameterized query)
    let injections = [
        "'; DROP TABLE primes; --",
        "%'; DELETE FROM primes WHERE '1'='1",
        "' OR '1'='1",
        "\\'; UPDATE primes SET form='hacked'; --",
    ];

    for injection in &injections {
        let uri = format!("/api/primes?search={}", urlencoding::encode(injection));
        let (status, json) = get(app().await, &uri).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "Search injection should not crash: {}",
            injection
        );
        assert!(
            json.is_array(),
            "Should return valid JSON for: {}",
            injection
        );
    }
}

// ---------------------------------------------------------------------------
// Body size limit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn body_size_limit_enforced() {
    require_db!();
    let router = app().await;

    // 2MB payload exceeds the 1MB limit
    let large_body = "x".repeat(2 * 1024 * 1024);
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/worker/register")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(large_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

// ---------------------------------------------------------------------------
// CORS
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cors_preflight_returns_correct_headers() {
    require_db!();
    let router = app().await;

    // Send a CORS preflight request (OPTIONS)
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/stats")
                .method(Method::OPTIONS)
                .header("origin", "https://evil.example.com")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should have CORS headers
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_some(),
        "Missing access-control-allow-origin header"
    );
    assert!(
        response
            .headers()
            .get("access-control-allow-methods")
            .is_some(),
        "Missing access-control-allow-methods header"
    );
}

#[tokio::test]
async fn cors_get_includes_allow_origin() {
    require_db!();
    let router = app().await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .header("origin", "http://localhost:3000")
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

// ---------------------------------------------------------------------------
// Path traversal
// ---------------------------------------------------------------------------

#[tokio::test]
async fn path_traversal_in_doc_slug_rejected() {
    require_db!();
    let traversal_attempts = [
        "/api/docs/../../../etc/passwd",
        "/api/docs/..%2F..%2Fetc%2Fpasswd",
        "/api/docs/foo\\..\\..\\etc\\passwd",
        "/api/docs/..\\windows\\system32",
    ];

    for path in &traversal_attempts {
        let (status, _) = get(app().await, path).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
            "Path traversal should be rejected: {} (got {})",
            path,
            status
        );
    }
}

#[tokio::test]
async fn path_traversal_in_roadmap_slug_rejected() {
    require_db!();
    let (status, _) = get(app().await, "/api/docs/roadmaps/../../../etc/passwd").await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
        "Roadmap path traversal should be rejected (got {})",
        status
    );
}

// ---------------------------------------------------------------------------
// Request validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_job_negative_block_size_rejected() {
    require_db!();
    let router = app().await;

    let payload = serde_json::json!({
        "search_type": "factorial",
        "params": {},
        "range_start": 1,
        "range_end": 100,
        "block_size": -5
    });

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/search_jobs")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn malformed_json_returns_error() {
    require_db!();
    let router = app().await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/worker/register")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from("{invalid json}"))
                .unwrap(),
        )
        .await
        .unwrap();
    // Should return 4xx (400 or 422)
    assert!(
        response.status().is_client_error(),
        "Malformed JSON should return client error, got {}",
        response.status()
    );
}
