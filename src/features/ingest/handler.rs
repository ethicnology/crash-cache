use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use diesel::prelude::*;
use diesel::sql_query;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use crate::shared::domain::DomainError;
use crate::shared::parser::{Envelope, SentrySession};
use crate::shared::persistence::db::models::NewSessionModel;
use crate::shared::persistence::{
    DbPool, ProjectRepository, SessionRepository, UnwrapSessionEnvironmentRepository,
    UnwrapSessionReleaseRepository, UnwrapSessionStatusRepository,
};

use super::IngestReportUseCase;

/// Maps DomainError to appropriate HTTP status codes and JSON responses
fn map_domain_error_to_response(error: &DomainError) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        DomainError::ProjectNotFound(pid) => {
            warn!(project_id = %pid, "Project not found");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("Project {} not found", pid)})),
            )
        }
        DomainError::DuplicateEventId(event_id) => {
            debug!(event_id = %event_id, "Duplicate event");
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "Duplicate event", "event_id": event_id})),
            )
        }
        DomainError::Database(msg) => {
            error!(error = %msg, "Database error");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Database temporarily unavailable"})),
            )
        }
        DomainError::ConnectionPool(msg) => {
            error!(error = %msg, "Connection pool exhausted");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Service temporarily unavailable"})),
            )
        }
        DomainError::Serialization(msg) => {
            error!(error = %msg, "Serialization error");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": "Invalid data format"})),
            )
        }
        DomainError::Compression(msg) | DomainError::Decompression(msg) => {
            warn!(error = %msg, "Compression/decompression error");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": "Invalid payload compression"})),
            )
        }
        DomainError::InvalidRequest(msg) => {
            warn!(error = %msg, "Invalid request");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": msg})),
            )
        }
        // Catch-all for other errors
        _ => {
            error!(error = %error, "Internal error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SentryQueryParams {
    pub sentry_key: Option<String>,
}

#[derive(Clone, Default)]
pub struct HealthStats {
    pub(crate) archives: i64,
    pub(crate) reports: i64,
    pub(crate) queue: i64,
    pub(crate) regurgitated: i64,
    pub(crate) orphaned: i64,
    #[allow(dead_code)]
    updated_at: Option<Instant>,
}

#[derive(Clone)]
pub struct ProjectCache {
    data: Arc<RwLock<HashMap<i32, (String, Instant)>>>, // project_id -> (public_key, cached_at)
    ttl: Duration,
}

impl ProjectCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    pub fn get(&self, project_id: i32) -> Option<String> {
        let cache = self.data.read().unwrap();
        if let Some((key, cached_at)) = cache.get(&project_id)
            && cached_at.elapsed() < self.ttl
        {
            return Some(key.clone());
        }
        None
    }

    pub fn insert(&self, project_id: i32, public_key: String) {
        let mut cache = self.data.write().unwrap();
        cache.insert(project_id, (public_key, Instant::now()));
    }
}

#[derive(Clone)]
pub struct AppState {
    pub ingest_use_case: IngestReportUseCase,
    pub compression_semaphore: Arc<Semaphore>,
    pub pool: DbPool,
    pub project_repo: ProjectRepository,
    pub project_cache: ProjectCache,
    pub health_cache: Arc<RwLock<HealthStats>>,
    pub health_cache_ttl: Duration,
    pub max_uncompressed_payload_bytes: usize,
    // Session repositories
    pub session_repo: SessionRepository,
    pub session_status_repo: UnwrapSessionStatusRepository,
    pub session_release_repo: UnwrapSessionReleaseRepository,
    pub session_environment_repo: UnwrapSessionEnvironmentRepository,
}

/// Creates the API router (rate-limited routes)
pub fn create_api_router(state: AppState) -> Router {
    Router::new()
        .route("/api/{project_id}/store/", post(store_report))
        .route("/api/{project_id}/store", post(store_report))
        .route("/api/{project_id}/envelope/", post(envelope_report))
        .route("/api/{project_id}/envelope", post(envelope_report))
        .with_state(state)
}

/// Creates the health router (no rate limiting)
pub fn create_health_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .with_state(state)
}

async fn store_report(
    State(state): State<AppState>,
    Path(project_id): Path<i32>,
    Query(query): Query<SentryQueryParams>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let start = std::time::Instant::now();

    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received store report"
    );

    // Get connection first
    let mut conn = match state.pool.get() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to get DB connection");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Service temporarily unavailable"})),
            );
        }
    };

    // Validate sentry_key
    let sentry_key = extract_sentry_key(&headers, &query);
    if let Err(response) = validate_project_key(
        &state.project_repo,
        &state.project_cache,
        &mut conn,
        project_id,
        sentry_key,
    ) {
        return response;
    }

    let (hash, compressed, original_size) = match prepare_payload(
        &headers,
        &body,
        &state.compression_semaphore,
        state.max_uncompressed_payload_bytes,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };

    match state.ingest_use_case.execute(
        &mut conn,
        project_id,
        hash.clone(),
        compressed,
        original_size,
    ) {
        Ok(_) => {
            let duration_ms = start.elapsed().as_millis();
            info!(
                project_id = %project_id,
                hash = %hash,
                status = 200,
                duration_ms = duration_ms,
                "Report stored successfully"
            );
            (StatusCode::OK, Json(serde_json::json!({"id": hash})))
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis();
            let response = map_domain_error_to_response(&e);
            warn!(
                project_id = %project_id,
                status = response.0.as_u16(),
                duration_ms = duration_ms,
                error = ?e,
                "Report storage failed"
            );
            response
        }
    }
}

async fn envelope_report(
    State(state): State<AppState>,
    Path(project_id): Path<i32>,
    Query(query): Query<SentryQueryParams>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let start = std::time::Instant::now();

    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received envelope report"
    );

    // Get connection first
    let mut conn = match state.pool.get() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to get DB connection");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Service temporarily unavailable"})),
            );
        }
    };

    // Validate sentry_key
    let sentry_key = extract_sentry_key(&headers, &query);
    if let Err(response) = validate_project_key(
        &state.project_repo,
        &state.project_cache,
        &mut conn,
        project_id,
        sentry_key,
    ) {
        return response;
    }

    let (hash, compressed, original_size) = match prepare_payload(
        &headers,
        &body,
        &state.compression_semaphore,
        state.max_uncompressed_payload_bytes,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };

    let decompressed = match decompress(&compressed) {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "Failed to decompress for parsing");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid gzip payload"})),
            );
        }
    };

    let envelope = match Envelope::parse(&decompressed) {
        Some(e) => e,
        None => {
            warn!("Failed to parse envelope format");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid envelope format"})),
            );
        }
    };

    // Check for event payload
    let has_event = envelope.find_event_payload().is_some();

    if !has_event {
        // Session-only envelope - process sessions immediately (no archive/queue)
        let session_payloads = envelope.find_session_payloads();
        let mut sessions_stored = 0;
        let mut first_error: Option<DomainError> = None;

        for session_data in session_payloads {
            if let Some(session) = SentrySession::parse(session_data) {
                match store_session(&state, &mut conn, project_id, &session) {
                    Ok(_sid_id) => {
                        sessions_stored += 1;
                    }
                    Err(e) => {
                        warn!(error = %e, sid = %session.sid, "Failed to store session");
                        if first_error.is_none() {
                            first_error = Some(e);
                        }
                    }
                }
            }
        }

        if sessions_stored > 0 {
            let duration_ms = start.elapsed().as_millis();
            info!(
                project_id = %project_id,
                sessions_stored = sessions_stored,
                status = 200,
                duration_ms = duration_ms,
                "Session-only envelope processed"
            );
            return (
                StatusCode::OK,
                Json(serde_json::json!({"sessions": sessions_stored})),
            );
        }

        // If we had errors but no successes, return the error
        if let Some(error) = first_error {
            let duration_ms = start.elapsed().as_millis();
            let response = map_domain_error_to_response(&error);
            warn!(
                project_id = %project_id,
                status = response.0.as_u16(),
                duration_ms = duration_ms,
                error = ?error,
                "Session processing failed"
            );
            return response;
        }

        let duration_ms = start.elapsed().as_millis();
        warn!(
            project_id = %project_id,
            status = 400,
            duration_ms = duration_ms,
            "No event or session found in envelope"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No event or session in envelope"})),
        );
    }

    // Event envelope - archive it (sessions will be processed during digest)
    match state.ingest_use_case.execute(
        &mut conn,
        project_id,
        hash.clone(),
        compressed,
        original_size,
    ) {
        Ok(_) => {
            let duration_ms = start.elapsed().as_millis();
            info!(
                project_id = %project_id,
                hash = %hash,
                status = 200,
                duration_ms = duration_ms,
                "Envelope archived for digest"
            );
            (StatusCode::OK, Json(serde_json::json!({"id": hash})))
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis();
            let response = map_domain_error_to_response(&e);
            warn!(
                project_id = %project_id,
                status = response.0.as_u16(),
                duration_ms = duration_ms,
                error = ?e,
                "Envelope storage failed"
            );
            response
        }
    }
}

/// Stores a session and returns the session_id for linking with reports
fn store_session(
    state: &AppState,
    conn: &mut crate::shared::persistence::DbConnection,
    project_id: i32,
    session: &SentrySession,
) -> Result<i32, DomainError> {
    // Get or create status ID
    let status_id = state
        .session_status_repo
        .get_or_create(conn, &session.status)?;

    // Get or create release ID (optional)
    let release_id = match &session.attrs.release {
        Some(r) => Some(state.session_release_repo.get_or_create(conn, r)?),
        None => None,
    };

    // Get or create environment ID (optional)
    let environment_id = match &session.attrs.environment {
        Some(env) => Some(state.session_environment_repo.get_or_create(conn, env)?),
        None => None,
    };

    let new_session = NewSessionModel {
        project_id,
        sid: session.sid.clone(),
        init: if session.init { 1 } else { 0 },
        started_at: session.started.clone(),
        timestamp: session
            .timestamp
            .clone()
            .unwrap_or_else(|| session.started.clone()),
        errors: session.errors,
        status_id,
        release_id,
        environment_id,
    };

    let session_id = state.session_repo.upsert(conn, new_session)?;

    Ok(session_id)
}

async fn prepare_payload(
    headers: &HeaderMap,
    body: &[u8],
    semaphore: &Semaphore,
    max_size: usize,
) -> Result<(String, Vec<u8>, Option<i32>), (StatusCode, Json<serde_json::Value>)> {
    let is_gzip = headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("gzip"))
        .unwrap_or(false);

    if is_gzip {
        let hash = compute_hash(body);
        Ok((hash, body.to_vec(), None))
    } else {
        if body.len() > max_size {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({
                    "error": format!("Payload too large: {} bytes (max {})", body.len(), max_size)
                })),
            ));
        }

        let permit = semaphore.try_acquire();
        if permit.is_err() {
            warn!("Compression semaphore exhausted - service overloaded");
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Service overloaded, please retry"})),
            ));
        }

        let original_size = body.len() as i32;
        let body_clone = body.to_vec();
        let compressed = tokio::task::spawn_blocking(move || compress(&body_clone))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;

        let hash = compute_hash(&compressed);
        Ok((hash, compressed, Some(original_size)))
    }
}

fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn compress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data)?;
    encoder.finish()
}

fn decompress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// Extracts sentry_key from X-Sentry-Auth header or query params.
/// Header format: "Sentry sentry_key=abc123, sentry_version=7, ..."
fn extract_sentry_key(headers: &HeaderMap, query: &SentryQueryParams) -> Option<String> {
    // Try query param first
    if let Some(key) = &query.sentry_key {
        return Some(key.clone());
    }

    // Try X-Sentry-Auth header
    if let Some(auth_header) = headers.get("X-Sentry-Auth").and_then(|v| v.to_str().ok()) {
        for part in auth_header.split(',') {
            let part = part.trim();
            if let Some(key) = part.strip_prefix("Sentry sentry_key=") {
                return Some(key.to_string());
            }
            if let Some(key) = part.strip_prefix("sentry_key=") {
                return Some(key.to_string());
            }
        }
    }

    None
}

fn validate_project_key(
    project_repo: &ProjectRepository,
    project_cache: &ProjectCache,
    conn: &mut crate::shared::persistence::DbConnection,
    project_id: i32,
    sentry_key: Option<String>,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let key = match sentry_key {
        Some(k) => k,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing sentry_key"})),
            ));
        }
    };

    // Check cache first
    if let Some(cached_key) = project_cache.get(project_id)
        && cached_key == key
    {
        return Ok(());
    }
    // Cached key doesn't match or cache miss - fall through to DB validation

    match project_repo.validate_key(conn, project_id, &key) {
        Ok(true) => {
            // Valid - update cache
            project_cache.insert(project_id, key);
            Ok(())
        }
        Ok(false) => {
            warn!(project_id = %project_id, received_key = %key, "Invalid public key");
            Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid public key"})),
            ))
        }
        Err(e) => Err(map_domain_error_to_response(&e)),
    }
}

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let cache = state.health_cache.read().unwrap();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "service": "crash-cache",
            "stats": {
                "ingested": cache.archives,
                "digested": cache.reports,
                "queued": cache.queue,
                "regurgitated": cache.regurgitated,
                "orphaned": cache.orphaned
            }
        })),
    )
}

pub fn compute_health_stats(conn: &mut crate::shared::persistence::DbConnection) -> HealthStats {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        c: i64,
    }

    let archives = sql_query("SELECT COUNT(*) as c FROM archive")
        .get_result::<Count>(conn)
        .map(|r| r.c)
        .map_err(|e| {
            warn!(error = %e, "Failed to query archive count");
        })
        .unwrap_or(0);

    let reports = sql_query("SELECT COUNT(*) as c FROM report")
        .get_result::<Count>(conn)
        .map(|r| r.c)
        .map_err(|e| {
            warn!(error = %e, "Failed to query report count");
        })
        .unwrap_or(0);

    let queue = sql_query("SELECT COUNT(*) as c FROM queue")
        .get_result::<Count>(conn)
        .map(|r| r.c)
        .map_err(|e| {
            warn!(error = %e, "Failed to query queue count");
        })
        .unwrap_or(0);

    let regurgitated = sql_query("SELECT COUNT(*) as c FROM queue_error")
        .get_result::<Count>(conn)
        .map(|r| r.c)
        .map_err(|e| {
            warn!(error = %e, "Failed to query queue_error count");
        })
        .unwrap_or(0);

    // Calculate orphaned instead of querying (much faster!)
    // Orphaned = archives not in reports, queue, or queue_error
    let orphaned = archives - reports - queue - regurgitated;

    HealthStats {
        archives,
        reports,
        queue,
        regurgitated,
        orphaned,
        updated_at: Some(Instant::now()),
    }
}
