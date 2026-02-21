//! # Mock Coordinator — Simulated Coordinator HTTP Server for Tests
//!
//! Provides a lightweight, in-process HTTP server that mimics the darkreach coordinator's
//! operator API. This enables integration testing of the operator/worker client code
//! (`src/operator.rs`) without a real database or coordinator process.
//!
//! ## Supported Endpoints
//!
//! | Method | Path                        | Purpose                        |
//! |--------|-----------------------------|--------------------------------|
//! | POST   | `/api/v1/register`          | Register a volunteer account   |
//! | POST   | `/api/v1/worker/register`   | Register a worker node         |
//! | POST   | `/api/v1/worker/heartbeat`  | Worker heartbeat               |
//! | GET    | `/api/v1/work`              | Claim a work block             |
//! | POST   | `/api/v1/result`            | Submit computation results     |
//! | GET    | `/api/v1/worker/latest`     | Check for worker updates       |
//!
//! ## Architecture
//!
//! ```text
//! MockCoordinator::start()
//!   └─ TcpListener::bind("127.0.0.1:0")   (random port)
//!   └─ axum::serve(listener, router)       (background tokio task)
//!   └─ SharedState (Arc<Mutex<MockState>>) (configurable responses + request log)
//!
//! Test code                          Mock server
//! ┌──────────────┐                  ┌──────────────────────┐
//! │ mock.url()    │ ──────────────> │ 127.0.0.1:<port>     │
//! │ operator::*() │ ──HTTP req───> │ route handler          │
//! │               │ <──HTTP res──  │  └─ reads MockState    │
//! │ mock.heartbeats() ──────────>  │  └─ records request    │
//! └──────────────┘                  └──────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mock_coordinator::MockCoordinator;
//!
//! #[tokio::test]
//! async fn test_operator_registers() {
//!     let mock = MockCoordinator::start().await;
//!     let result = darkreach::operator::register(&mock.url(), "alice", "alice@example.com");
//!     assert!(result.is_ok());
//!
//!     // Verify the server received the registration
//!     let workers = mock.registered_workers();
//!     assert_eq!(workers.len(), 0); // register != worker register
//! }
//!
//! #[tokio::test]
//! async fn test_claim_work_when_available() {
//!     let mock = MockCoordinator::builder()
//!         .with_work(WorkAssignmentConfig {
//!             block_id: 42,
//!             search_job_id: 7,
//!             search_type: "factorial".to_string(),
//!             params: serde_json::json!({"start": 1, "end": 100}),
//!             block_start: 1,
//!             block_end: 50,
//!         })
//!         .start()
//!         .await;
//!
//!     // operator::claim_work will receive the configured work assignment
//! }
//!
//! #[tokio::test]
//! async fn test_claim_work_when_empty() {
//!     let mock = MockCoordinator::builder()
//!         .with_no_work()
//!         .start()
//!         .await;
//!
//!     // operator::claim_work will receive 204 No Content -> None
//! }
//! ```
//!
//! ## Thread Safety
//!
//! All shared state is behind `Arc<Mutex<...>>`, making the mock safe for concurrent
//! requests. The mutex is held only briefly during state reads/writes, so contention
//! is negligible in test scenarios.

#![allow(dead_code)]

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

// ── Configuration Types ─────────────────────────────────────────────

/// Configuration for a work assignment the mock will return.
///
/// Maps 1:1 to the JSON payload returned by `GET /api/v1/work` in the
/// real coordinator (see `routes_operator::handler_v1_work`).
#[derive(Debug, Clone, Serialize)]
pub struct WorkAssignmentConfig {
    pub block_id: i64,
    pub search_job_id: i64,
    pub search_type: String,
    pub params: serde_json::Value,
    pub block_start: i64,
    pub block_end: i64,
}

/// Configuration for a worker release the mock will return.
///
/// Maps to the JSON payload returned by `GET /api/v1/worker/latest`
/// (see `routes_operator::handler_worker_latest`).
#[derive(Debug, Clone, Serialize)]
pub struct WorkerReleaseConfig {
    pub channel: String,
    pub version: String,
    pub published_at: String,
    pub notes: Option<String>,
    pub artifacts: Vec<WorkerReleaseArtifactConfig>,
}

/// A single release artifact within a `WorkerReleaseConfig`.
#[derive(Debug, Clone, Serialize)]
pub struct WorkerReleaseArtifactConfig {
    pub os: String,
    pub arch: String,
    pub url: String,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig_url: Option<String>,
}

// ── Recorded Request Types ──────────────────────────────────────────

/// A worker registration request recorded by the mock.
///
/// Captured from `POST /api/v1/worker/register` payloads for post-test assertions.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordedWorkerRegistration {
    pub worker_id: String,
    pub hostname: String,
    pub cores: i32,
    #[serde(default)]
    pub cpu_model: String,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub ram_gb: Option<i32>,
    #[serde(default)]
    pub has_gpu: Option<bool>,
    #[serde(default)]
    pub gpu_model: Option<String>,
    #[serde(default)]
    pub gpu_vram_gb: Option<i32>,
    #[serde(default)]
    pub worker_version: Option<String>,
    #[serde(default)]
    pub update_channel: Option<String>,
    /// The Bearer token from the Authorization header.
    pub auth_token: Option<String>,
}

/// A heartbeat request recorded by the mock.
///
/// Captured from `POST /api/v1/worker/heartbeat` payloads.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordedHeartbeat {
    pub worker_id: String,
    /// The Bearer token from the Authorization header.
    pub auth_token: Option<String>,
}

/// A result submission recorded by the mock.
///
/// Captured from `POST /api/v1/result` payloads.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordedResultSubmission {
    pub block_id: i64,
    pub tested: i64,
    pub found: i64,
    pub primes: Vec<RecordedPrimeReport>,
    /// The Bearer token from the Authorization header.
    pub auth_token: Option<String>,
}

/// A prime report within a recorded result submission.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordedPrimeReport {
    pub expression: String,
    pub form: String,
    pub digits: u64,
    pub proof_method: String,
    #[serde(default)]
    pub certificate: Option<String>,
}

/// A volunteer registration recorded by the mock.
///
/// Captured from `POST /api/v1/register` payloads.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordedRegistration {
    pub username: String,
    pub email: String,
}

// ── Configurable Error Behavior ─────────────────────────────────────

/// Controls what the mock returns for a specific endpoint.
///
/// `Normal` means the endpoint behaves according to its default logic.
/// `Error(status)` means the endpoint returns the given HTTP status with
/// a JSON error body, useful for testing error-handling paths in the client.
#[derive(Debug, Clone)]
pub enum MockBehavior {
    /// Return the normal, successful response.
    Normal,
    /// Return an HTTP error with the given status code.
    Error(u16),
}

// ── Shared Mock State ───────────────────────────────────────────────

/// Interior mutable state shared between the mock's route handlers and the
/// test code that inspects it. Protected by `Mutex` for thread safety.
///
/// All vectors grow monotonically during a test (append-only). The test reads
/// them after exercising the client code to verify correct request payloads.
#[derive(Debug)]
struct MockState {
    /// If `Some`, the mock returns this work assignment for `GET /api/v1/work`.
    /// If `None`, returns 204 No Content (no work available).
    work_assignment: Option<WorkAssignmentConfig>,

    /// If `Some`, the mock returns this release info for `GET /api/v1/worker/latest`.
    /// If `None`, returns 404 (no update available).
    worker_release: Option<WorkerReleaseConfig>,

    /// Override behavior for `POST /api/v1/register`.
    register_behavior: MockBehavior,

    /// Override behavior for `GET /api/v1/work`.
    claim_behavior: MockBehavior,

    /// Override behavior for `GET /api/v1/worker/latest`.
    update_behavior: MockBehavior,

    /// The API key the mock accepts as valid for Bearer token auth.
    /// Defaults to `"test-api-key-mock"`.
    valid_api_key: String,

    /// Counter for generating unique API keys on registration.
    register_counter: u64,

    // ── Request logs ────────────────────────────────────────────

    /// All `POST /api/v1/register` payloads received.
    registrations: Vec<RecordedRegistration>,

    /// All `POST /api/v1/worker/register` payloads received.
    worker_registrations: Vec<RecordedWorkerRegistration>,

    /// All `POST /api/v1/worker/heartbeat` payloads received.
    heartbeats: Vec<RecordedHeartbeat>,

    /// All `POST /api/v1/result` payloads received.
    result_submissions: Vec<RecordedResultSubmission>,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            work_assignment: None,
            worker_release: None,
            register_behavior: MockBehavior::Normal,
            claim_behavior: MockBehavior::Normal,
            update_behavior: MockBehavior::Normal,
            valid_api_key: "test-api-key-mock".to_string(),
            register_counter: 0,
            registrations: Vec::new(),
            worker_registrations: Vec::new(),
            heartbeats: Vec::new(),
            result_submissions: Vec::new(),
        }
    }
}

type SharedState = Arc<Mutex<MockState>>;

// ── MockCoordinator ─────────────────────────────────────────────────

/// A mock coordinator HTTP server for testing the operator client.
///
/// Starts an axum HTTP server on a random localhost port. The server runs
/// in a background tokio task and is automatically shut down when this
/// struct is dropped (via `AbortHandle`).
///
/// Use `MockCoordinator::start()` for defaults, or `MockCoordinator::builder()`
/// to configure custom responses before starting the server.
pub struct MockCoordinator {
    /// The base URL of the running mock server (e.g., `http://127.0.0.1:12345`).
    base_url: String,
    /// Handle to abort the background server task on drop.
    _abort_handle: tokio::task::AbortHandle,
    /// Shared state for reading recorded requests and reconfiguring responses.
    state: SharedState,
}

impl MockCoordinator {
    /// Start a mock coordinator with default configuration.
    ///
    /// - Registration succeeds and returns `test-api-key-mock-N`.
    /// - Worker registration/heartbeat/result succeed with 200 OK.
    /// - Work claim returns 204 No Content (no work).
    /// - Update check returns 404 (no update).
    /// - Auth accepts `Bearer test-api-key-mock`.
    pub async fn start() -> Self {
        Self::builder().start().await
    }

    /// Create a builder for configuring the mock before starting.
    pub fn builder() -> MockCoordinatorBuilder {
        MockCoordinatorBuilder {
            state: MockState::default(),
        }
    }

    /// Returns the base URL of the running mock server.
    ///
    /// Example: `"http://127.0.0.1:54321"`. Pass this to `operator::register()`,
    /// `operator::claim_work()`, etc. as the `server` argument.
    pub fn url(&self) -> String {
        self.base_url.clone()
    }

    /// Returns the API key the mock considers valid for Bearer authentication.
    ///
    /// Defaults to `"test-api-key-mock"`. Use this when constructing an
    /// `OperatorConfig` for test calls to authenticated endpoints.
    pub fn valid_api_key(&self) -> String {
        self.state.lock().unwrap().valid_api_key.clone()
    }

    // ── Request inspection ──────────────────────────────────────

    /// Returns all volunteer registration requests received by the mock.
    ///
    /// These are `POST /api/v1/register` payloads. Each entry contains
    /// the `username` and `email` from the request body.
    pub fn registrations(&self) -> Vec<RecordedRegistration> {
        self.state.lock().unwrap().registrations.clone()
    }

    /// Returns all worker registration requests received by the mock.
    ///
    /// These are `POST /api/v1/worker/register` payloads including hardware
    /// capability fields and the Bearer token used for authentication.
    pub fn registered_workers(&self) -> Vec<RecordedWorkerRegistration> {
        self.state.lock().unwrap().worker_registrations.clone()
    }

    /// Returns all heartbeat requests received by the mock.
    ///
    /// These are `POST /api/v1/worker/heartbeat` payloads. Each entry
    /// contains the `worker_id` and the Bearer token used for authentication.
    pub fn heartbeats(&self) -> Vec<RecordedHeartbeat> {
        self.state.lock().unwrap().heartbeats.clone()
    }

    /// Returns all result submissions received by the mock.
    ///
    /// These are `POST /api/v1/result` payloads including block_id,
    /// tested/found counts, any discovered primes, and the Bearer token.
    pub fn submitted_results(&self) -> Vec<RecordedResultSubmission> {
        self.state.lock().unwrap().result_submissions.clone()
    }

    // ── Runtime reconfiguration ─────────────────────────────────

    /// Configure the mock to return a specific work assignment for the next
    /// `GET /api/v1/work` request(s).
    pub fn set_work(&self, assignment: WorkAssignmentConfig) {
        self.state.lock().unwrap().work_assignment = Some(assignment);
    }

    /// Configure the mock to return 204 No Content for `GET /api/v1/work`.
    pub fn set_no_work(&self) {
        self.state.lock().unwrap().work_assignment = None;
    }

    /// Configure the mock to return a specific release for `GET /api/v1/worker/latest`.
    pub fn set_update(&self, release: WorkerReleaseConfig) {
        self.state.lock().unwrap().worker_release = Some(release);
    }

    /// Configure the mock to return 404 for `GET /api/v1/worker/latest`.
    pub fn set_no_update(&self) {
        self.state.lock().unwrap().worker_release = None;
    }
}

// ── Builder ─────────────────────────────────────────────────────────

/// Builder for `MockCoordinator` that allows configuring responses before
/// starting the server.
pub struct MockCoordinatorBuilder {
    state: MockState,
}

impl MockCoordinatorBuilder {
    /// Configure a work assignment to return from `GET /api/v1/work`.
    pub fn with_work(mut self, assignment: WorkAssignmentConfig) -> Self {
        self.state.work_assignment = Some(assignment);
        self.state.claim_behavior = MockBehavior::Normal;
        self
    }

    /// Configure the mock to return 204 No Content from `GET /api/v1/work`.
    pub fn with_no_work(mut self) -> Self {
        self.state.work_assignment = None;
        self.state.claim_behavior = MockBehavior::Normal;
        self
    }

    /// Configure a worker release to return from `GET /api/v1/worker/latest`.
    pub fn with_update(mut self, release: WorkerReleaseConfig) -> Self {
        self.state.worker_release = Some(release);
        self.state.update_behavior = MockBehavior::Normal;
        self
    }

    /// Configure the mock to return 404 from `GET /api/v1/worker/latest`.
    pub fn with_no_update(mut self) -> Self {
        self.state.worker_release = None;
        self.state.update_behavior = MockBehavior::Normal;
        self
    }

    /// Configure `POST /api/v1/register` to return an HTTP error.
    ///
    /// Common values:
    /// - `409` — duplicate username/email (CONFLICT)
    /// - `500` — internal server error
    /// - `400` — bad request (validation failure)
    pub fn with_register_error(mut self, status: u16) -> Self {
        self.state.register_behavior = MockBehavior::Error(status);
        self
    }

    /// Configure `GET /api/v1/work` to return an HTTP error.
    ///
    /// Common values:
    /// - `500` — internal server error
    /// - `401` — unauthorized
    pub fn with_claim_error(mut self, status: u16) -> Self {
        self.state.claim_behavior = MockBehavior::Error(status);
        self
    }

    /// Configure `GET /api/v1/worker/latest` to return an HTTP error.
    ///
    /// Common values:
    /// - `503` — service unavailable (manifest missing)
    /// - `500` — internal server error
    pub fn with_update_error(mut self, status: u16) -> Self {
        self.state.update_behavior = MockBehavior::Error(status);
        self
    }

    /// Set the API key the mock accepts as valid for Bearer authentication.
    ///
    /// Defaults to `"test-api-key-mock"`. If you want to test auth failures,
    /// set this to something the client will not send.
    pub fn with_valid_api_key(mut self, key: impl Into<String>) -> Self {
        self.state.valid_api_key = key.into();
        self
    }

    /// Build and start the mock coordinator server.
    ///
    /// Binds to `127.0.0.1:0` (OS-assigned random port), spawns the axum
    /// server as a background tokio task, and returns the `MockCoordinator`
    /// handle with the assigned port in its base URL.
    pub async fn start(self) -> MockCoordinator {
        let shared_state: SharedState = Arc::new(Mutex::new(self.state));

        let app = Router::new()
            .route("/api/v1/register", post(handle_register))
            .route("/api/v1/worker/register", post(handle_worker_register))
            .route("/api/v1/worker/heartbeat", post(handle_heartbeat))
            .route("/api/v1/work", get(handle_work))
            .route("/api/v1/result", post(handle_result))
            .route("/api/v1/worker/latest", get(handle_worker_latest))
            .with_state(Arc::clone(&shared_state));

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind mock coordinator to random port");
        let addr: SocketAddr = listener
            .local_addr()
            .expect("Failed to get mock coordinator local address");
        let base_url = format!("http://127.0.0.1:{}", addr.port());

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("Mock coordinator server failed");
        });

        MockCoordinator {
            base_url,
            _abort_handle: handle.abort_handle(),
            state: shared_state,
        }
    }
}

// ── Helper: extract Bearer token ────────────────────────────────────

/// Extract the Bearer token from an Authorization header, if present.
fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

/// Validate the Bearer token against the mock's configured valid API key.
/// Returns `Ok(token)` if valid, or an unauthorized error response.
fn validate_auth(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let token = extract_bearer_token(headers).unwrap_or_default();
    let valid_key = state.lock().unwrap().valid_api_key.clone();
    if token.is_empty() || token != valid_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Missing or invalid Authorization header"})),
        ));
    }
    Ok(token)
}

// ── Route Handlers ──────────────────────────────────────────────────

/// `POST /api/v1/register` — Register a volunteer.
///
/// Normal behavior: returns 201 Created with `{ api_key, username }`.
/// The API key is `"test-api-key-mock-N"` where N is a monotonically
/// increasing counter (unique per mock instance).
///
/// Error behavior: returns the configured HTTP status with `{ error }`.
async fn handle_register(
    State(state): State<SharedState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let username = payload["username"].as_str().unwrap_or("").to_string();
    let email = payload["email"].as_str().unwrap_or("").to_string();

    // Record the request before checking behavior.
    {
        let mut s = state.lock().unwrap();
        s.registrations.push(RecordedRegistration {
            username: username.clone(),
            email: email.clone(),
        });
    }

    // Check for configured error behavior.
    let behavior = state.lock().unwrap().register_behavior.clone();
    if let MockBehavior::Error(code) = behavior {
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        return (
            status,
            Json(serde_json::json!({"error": format!("Mock error {}", code)})),
        );
    }

    // Basic validation (mirrors real coordinator).
    if username.len() < 3 || username.len() > 32 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Username must be 3-32 characters"})),
        );
    }
    if !email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid email address"})),
        );
    }

    // Generate a unique API key.
    let api_key = {
        let mut s = state.lock().unwrap();
        s.register_counter += 1;
        format!("test-api-key-mock-{}", s.register_counter)
    };

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "api_key": api_key,
            "username": username,
        })),
    )
}

/// `POST /api/v1/worker/register` — Register a worker node.
///
/// Requires Bearer token authentication. Records the full worker capability
/// payload for post-test inspection.
async fn handle_worker_register(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = match validate_auth(&state, &headers) {
        Ok(t) => t,
        Err(e) => return e,
    };

    let registration = RecordedWorkerRegistration {
        worker_id: payload["worker_id"].as_str().unwrap_or("").to_string(),
        hostname: payload["hostname"].as_str().unwrap_or("").to_string(),
        cores: payload["cores"].as_i64().unwrap_or(0) as i32,
        cpu_model: payload["cpu_model"].as_str().unwrap_or("").to_string(),
        os: payload["os"].as_str().map(|s| s.to_string()),
        arch: payload["arch"].as_str().map(|s| s.to_string()),
        ram_gb: payload["ram_gb"].as_i64().map(|v| v as i32),
        has_gpu: payload["has_gpu"].as_bool(),
        gpu_model: payload["gpu_model"].as_str().map(|s| s.to_string()),
        gpu_vram_gb: payload["gpu_vram_gb"].as_i64().map(|v| v as i32),
        worker_version: payload["worker_version"].as_str().map(|s| s.to_string()),
        update_channel: payload["update_channel"].as_str().map(|s| s.to_string()),
        auth_token: Some(token),
    };

    state.lock().unwrap().worker_registrations.push(registration);

    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

/// `POST /api/v1/worker/heartbeat` — Worker heartbeat.
///
/// Requires Bearer token authentication. Records the worker_id and token.
async fn handle_heartbeat(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = match validate_auth(&state, &headers) {
        Ok(t) => t,
        Err(e) => return e,
    };

    let heartbeat = RecordedHeartbeat {
        worker_id: payload["worker_id"].as_str().unwrap_or("").to_string(),
        auth_token: Some(token),
    };

    state.lock().unwrap().heartbeats.push(heartbeat);

    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

/// Query parameters for `GET /api/v1/work`.
#[derive(Debug, Deserialize)]
struct WorkQuery {
    #[serde(default)]
    cores: Option<usize>,
    #[serde(default)]
    ram_gb: Option<u64>,
    #[serde(default)]
    has_gpu: Option<bool>,
    #[serde(default)]
    os: Option<String>,
    #[serde(default)]
    arch: Option<String>,
}

/// `GET /api/v1/work` — Claim a work block.
///
/// Requires Bearer token authentication.
///
/// Normal behavior with work configured: returns 200 with the work assignment.
/// Normal behavior without work: returns 204 No Content.
/// Error behavior: returns the configured HTTP status.
async fn handle_work(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(_query): Query<WorkQuery>,
) -> impl IntoResponse {
    if let Err(e) = validate_auth(&state, &headers) {
        return e;
    }

    // Check for configured error behavior.
    let behavior = state.lock().unwrap().claim_behavior.clone();
    if let MockBehavior::Error(code) = behavior {
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        return (
            status,
            Json(serde_json::json!({"error": format!("Mock error {}", code)})),
        );
    }

    let assignment = state.lock().unwrap().work_assignment.clone();
    match assignment {
        Some(wa) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "block_id": wa.block_id,
                "search_job_id": wa.search_job_id,
                "search_type": wa.search_type,
                "params": wa.params,
                "block_start": wa.block_start,
                "block_end": wa.block_end,
            })),
        ),
        None => (
            StatusCode::NO_CONTENT,
            Json(serde_json::json!(null)),
        ),
    }
}

/// `POST /api/v1/result` — Submit computation results.
///
/// Requires Bearer token authentication. Records the full result payload
/// including any discovered primes.
async fn handle_result(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = match validate_auth(&state, &headers) {
        Ok(t) => t,
        Err(e) => return e,
    };

    let primes: Vec<RecordedPrimeReport> = payload["primes"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|p| RecordedPrimeReport {
            expression: p["expression"].as_str().unwrap_or("").to_string(),
            form: p["form"].as_str().unwrap_or("").to_string(),
            digits: p["digits"].as_u64().unwrap_or(0),
            proof_method: p["proof_method"].as_str().unwrap_or("").to_string(),
            certificate: p["certificate"].as_str().map(|s| s.to_string()),
        })
        .collect();

    let submission = RecordedResultSubmission {
        block_id: payload["block_id"].as_i64().unwrap_or(0),
        tested: payload["tested"].as_i64().unwrap_or(0),
        found: payload["found"].as_i64().unwrap_or(0),
        primes,
        auth_token: Some(token),
    };

    state.lock().unwrap().result_submissions.push(submission);

    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

/// Query parameters for `GET /api/v1/worker/latest`.
#[derive(Debug, Deserialize)]
struct LatestWorkerQuery {
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    worker_id: Option<String>,
}

/// `GET /api/v1/worker/latest` — Check for worker updates.
///
/// Does NOT require authentication (matches real coordinator behavior).
///
/// Normal behavior with release configured: returns 200 with release info.
/// Normal behavior without release: returns 404.
/// Error behavior: returns the configured HTTP status.
async fn handle_worker_latest(
    State(state): State<SharedState>,
    Query(query): Query<LatestWorkerQuery>,
) -> impl IntoResponse {
    // Check for configured error behavior.
    let behavior = state.lock().unwrap().update_behavior.clone();
    if let MockBehavior::Error(code) = behavior {
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        return (
            status,
            Json(serde_json::json!({"error": format!("Mock error {}", code)})),
        );
    }

    let release = state.lock().unwrap().worker_release.clone();
    let channel = query.channel.unwrap_or_else(|| "stable".to_string());

    match release {
        Some(rel) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "channel": channel,
                "version": rel.version,
                "published_at": rel.published_at,
                "notes": rel.notes,
                "artifacts": rel.artifacts,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Unknown release channel: {}", channel),
            })),
        ),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    //! Self-tests for the mock coordinator itself.
    //!
    //! These verify that the mock server starts, responds correctly to HTTP
    //! requests, records request payloads, and respects configured behaviors.
    //!
    //! Uses `reqwest` (async HTTP client) rather than `ureq` (blocking) to
    //! avoid deadlocks: `ureq` blocks the tokio thread, which also needs to
    //! service the mock server running on the same runtime. The real operator
    //! client uses `ureq`, but the wire format is identical.

    use super::*;

    /// Verify the mock starts and returns a valid URL with a port.
    #[tokio::test]
    async fn mock_starts_and_returns_url() {
        let mock = MockCoordinator::start().await;
        let url = mock.url();
        assert!(url.starts_with("http://127.0.0.1:"));
        let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
        assert!(port > 0);
    }

    /// Verify that POST /api/v1/register returns 201 with api_key and username.
    #[tokio::test]
    async fn register_returns_api_key() {
        let mock = MockCoordinator::start().await;
        let url = format!("{}/api/v1/register", mock.url());
        let body = serde_json::json!({"username": "alice", "email": "alice@example.com"});

        let client = reqwest::Client::new();
        let resp = client.post(&url).json(&body).send().await.unwrap();
        assert_eq!(resp.status(), 201);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert!(json["api_key"].as_str().unwrap().starts_with("test-api-key-mock-"));
        assert_eq!(json["username"].as_str().unwrap(), "alice");

        // Verify the registration was recorded.
        let regs = mock.registrations();
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].username, "alice");
        assert_eq!(regs[0].email, "alice@example.com");
    }

    /// Verify that POST /api/v1/register with a short username returns 400.
    #[tokio::test]
    async fn register_validates_username_length() {
        let mock = MockCoordinator::start().await;
        let url = format!("{}/api/v1/register", mock.url());
        let body = serde_json::json!({"username": "ab", "email": "ab@example.com"});

        let client = reqwest::Client::new();
        let resp = client.post(&url).json(&body).send().await.unwrap();
        assert_eq!(resp.status(), 400);
    }

    /// Verify that register error behavior is configurable.
    #[tokio::test]
    async fn register_error_configured() {
        let mock = MockCoordinator::builder()
            .with_register_error(409)
            .start()
            .await;
        let url = format!("{}/api/v1/register", mock.url());
        let body = serde_json::json!({"username": "alice", "email": "alice@example.com"});

        let client = reqwest::Client::new();
        let resp = client.post(&url).json(&body).send().await.unwrap();
        assert_eq!(resp.status(), 409);
    }

    /// Verify that POST /api/v1/worker/register requires auth and records payload.
    #[tokio::test]
    async fn worker_register_requires_auth() {
        let mock = MockCoordinator::start().await;
        let url = format!("{}/api/v1/worker/register", mock.url());
        let body = serde_json::json!({
            "worker_id": "test-worker-01",
            "hostname": "test-host",
            "cores": 8,
        });
        let client = reqwest::Client::new();

        // Without auth -> 401
        let resp = client.post(&url).json(&body).send().await.unwrap();
        assert_eq!(resp.status(), 401);

        // With auth -> 200
        let resp = client
            .post(&url)
            .bearer_auth(mock.valid_api_key())
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let workers = mock.registered_workers();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].worker_id, "test-worker-01");
        assert_eq!(workers[0].hostname, "test-host");
        assert_eq!(workers[0].cores, 8);
        assert_eq!(workers[0].auth_token, Some(mock.valid_api_key()));
    }

    /// Verify that POST /api/v1/worker/heartbeat records the worker_id.
    #[tokio::test]
    async fn heartbeat_records_worker_id() {
        let mock = MockCoordinator::start().await;
        let url = format!("{}/api/v1/worker/heartbeat", mock.url());
        let body = serde_json::json!({"worker_id": "w-12345678"});

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .bearer_auth(mock.valid_api_key())
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let hbs = mock.heartbeats();
        assert_eq!(hbs.len(), 1);
        assert_eq!(hbs[0].worker_id, "w-12345678");
    }

    /// Verify that GET /api/v1/work returns 204 when no work is configured.
    #[tokio::test]
    async fn work_returns_204_when_empty() {
        let mock = MockCoordinator::builder().with_no_work().start().await;
        let url = format!(
            "{}/api/v1/work?cores=4&ram_gb=16&has_gpu=false&os=linux&arch=x86_64",
            mock.url()
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(mock.valid_api_key())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);
    }

    /// Verify that GET /api/v1/work returns a work assignment when configured.
    #[tokio::test]
    async fn work_returns_assignment_when_configured() {
        let mock = MockCoordinator::builder()
            .with_work(WorkAssignmentConfig {
                block_id: 42,
                search_job_id: 7,
                search_type: "factorial".to_string(),
                params: serde_json::json!({"start": 1, "end": 100}),
                block_start: 1,
                block_end: 50,
            })
            .start()
            .await;
        let url = format!("{}/api/v1/work?cores=4", mock.url());

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(mock.valid_api_key())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["block_id"].as_i64().unwrap(), 42);
        assert_eq!(json["search_job_id"].as_i64().unwrap(), 7);
        assert_eq!(json["search_type"].as_str().unwrap(), "factorial");
        assert_eq!(json["block_start"].as_i64().unwrap(), 1);
        assert_eq!(json["block_end"].as_i64().unwrap(), 50);
    }

    /// Verify that GET /api/v1/work returns an error when configured.
    #[tokio::test]
    async fn work_returns_error_when_configured() {
        let mock = MockCoordinator::builder()
            .with_claim_error(500)
            .start()
            .await;
        let url = format!("{}/api/v1/work?cores=1", mock.url());

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(mock.valid_api_key())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 500);
    }

    /// Verify that POST /api/v1/result records the submission.
    #[tokio::test]
    async fn result_records_submission() {
        let mock = MockCoordinator::start().await;
        let url = format!("{}/api/v1/result", mock.url());
        let body = serde_json::json!({
            "block_id": 42,
            "tested": 1000,
            "found": 2,
            "primes": [{
                "expression": "7!+1",
                "form": "factorial",
                "digits": 4,
                "proof_method": "pocklington",
            }],
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .bearer_auth(mock.valid_api_key())
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let results = mock.submitted_results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].block_id, 42);
        assert_eq!(results[0].tested, 1000);
        assert_eq!(results[0].found, 2);
        assert_eq!(results[0].primes.len(), 1);
        assert_eq!(results[0].primes[0].expression, "7!+1");
        assert_eq!(results[0].primes[0].form, "factorial");
    }

    /// Verify that GET /api/v1/worker/latest returns 404 when no update is configured.
    #[tokio::test]
    async fn worker_latest_returns_404_when_no_update() {
        let mock = MockCoordinator::builder().with_no_update().start().await;
        let url = format!("{}/api/v1/worker/latest?channel=stable", mock.url());

        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), 404);
    }

    /// Verify that GET /api/v1/worker/latest returns release info when configured.
    #[tokio::test]
    async fn worker_latest_returns_release_when_configured() {
        let mock = MockCoordinator::builder()
            .with_update(WorkerReleaseConfig {
                channel: "stable".to_string(),
                version: "1.2.3".to_string(),
                published_at: "2026-02-20T12:00:00Z".to_string(),
                notes: Some("Bug fixes".to_string()),
                artifacts: vec![WorkerReleaseArtifactConfig {
                    os: "linux".to_string(),
                    arch: "x86_64".to_string(),
                    url: "https://example.com/darkreach-linux-x86_64.tar.gz".to_string(),
                    sha256: "abc123".to_string(),
                    sig_url: None,
                }],
            })
            .start()
            .await;
        let url = format!("{}/api/v1/worker/latest?channel=stable", mock.url());

        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), 200);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["version"].as_str().unwrap(), "1.2.3");
        assert_eq!(json["channel"].as_str().unwrap(), "stable");
        assert_eq!(json["notes"].as_str().unwrap(), "Bug fixes");
        let artifacts = json["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0]["os"].as_str().unwrap(), "linux");
    }

    /// Verify that GET /api/v1/worker/latest returns an error when configured.
    #[tokio::test]
    async fn worker_latest_returns_error_when_configured() {
        let mock = MockCoordinator::builder()
            .with_update_error(503)
            .start()
            .await;
        let url = format!("{}/api/v1/worker/latest?channel=stable", mock.url());

        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), 503);
    }

    /// Verify runtime reconfiguration via set_work / set_no_work.
    #[tokio::test]
    async fn runtime_reconfiguration_work() {
        let mock = MockCoordinator::start().await;
        let url = format!("{}/api/v1/work?cores=1", mock.url());
        let client = reqwest::Client::new();

        // Initially no work configured -> 204
        let resp = client
            .get(&url)
            .bearer_auth(mock.valid_api_key())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);

        // Configure work at runtime.
        mock.set_work(WorkAssignmentConfig {
            block_id: 99,
            search_job_id: 1,
            search_type: "kbn".to_string(),
            params: serde_json::json!({}),
            block_start: 100,
            block_end: 200,
        });

        // Now returns work -> 200
        let resp = client
            .get(&url)
            .bearer_auth(mock.valid_api_key())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["block_id"].as_i64().unwrap(), 99);

        // Clear work at runtime.
        mock.set_no_work();

        // Back to 204
        let resp = client
            .get(&url)
            .bearer_auth(mock.valid_api_key())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);
    }

    /// Verify that multiple requests accumulate in the recorded lists.
    #[tokio::test]
    async fn multiple_heartbeats_accumulate() {
        let mock = MockCoordinator::start().await;
        let url = format!("{}/api/v1/worker/heartbeat", mock.url());
        let client = reqwest::Client::new();

        for i in 0..5 {
            let body = serde_json::json!({"worker_id": format!("worker-{}", i)});
            let resp = client
                .post(&url)
                .bearer_auth(mock.valid_api_key())
                .json(&body)
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
        }

        let hbs = mock.heartbeats();
        assert_eq!(hbs.len(), 5);
        for i in 0..5 {
            assert_eq!(hbs[i].worker_id, format!("worker-{}", i));
        }
    }

    /// Verify that invalid auth returns 401 for authenticated endpoints.
    #[tokio::test]
    async fn invalid_auth_returns_401() {
        let mock = MockCoordinator::start().await;
        let client = reqwest::Client::new();

        // POST endpoints with wrong auth
        let post_endpoints = [
            format!("{}/api/v1/worker/register", mock.url()),
            format!("{}/api/v1/worker/heartbeat", mock.url()),
            format!("{}/api/v1/result", mock.url()),
        ];
        for url in &post_endpoints {
            let body = serde_json::json!({"worker_id": "test"});
            let resp = client
                .post(url)
                .bearer_auth("wrong-key")
                .json(&body)
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 401, "Expected 401 for POST {}", url);
        }

        // GET endpoints with wrong auth
        let get_url = format!("{}/api/v1/work?cores=1", mock.url());
        let resp = client
            .get(&get_url)
            .bearer_auth("wrong-key")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 401, "Expected 401 for GET /api/v1/work");
    }
}
