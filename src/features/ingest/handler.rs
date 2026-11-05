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
use crate::shared::parser::Envelope;
use crate::shared::persistence::{
    DbPool, ProjectRepository, SessionRepository, UnwrapSessionEnvironmentRepository,
    UnwrapSessionReleaseRepository, UnwrapSessionStatusRepository,
};

use super::use_case::IngestReportUseCase;

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
    let payload_size = body.len();

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

    let event_id = match decompress(&compressed).ok().and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok())
        .and_then(|event| event.get("event_id").and_then(|id| id.as_str()).map(str::to_owned))
        .and_then(|id| uuid::Uuid::parse_str(&id).ok()) {
        Some(id) => id.simple().to_string(),
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Event requires a valid event_id"}))),
    };

    match state.ingest_use_case.execute(
        &mut conn,
        project_id,
        hash.clone(),
        compressed,
        original_size,
    ) {
        Ok(result) => {
            if result.duplicate {
                warn!(
                    project_id = %project_id,
                    payload_size,
                    duration_ms = start.elapsed().as_millis(),
                    hash = %result.hash,
                    "Store DUP"
                );
            } else {
                info!(
                    project_id = %project_id,
                    payload_size,
                    duration_ms = start.elapsed().as_millis(),
                    hash = %result.hash,
                    "Store OK"
                );
            }
            (StatusCode::OK, Json(serde_json::json!({"id": event_id})))
        }
        Err(e) => {
            let response = map_domain_error_to_response(&e);
            warn!(
                project_id = %project_id,
                payload_size,
                status = response.0.as_u16(),
                duration_ms = start.elapsed().as_millis(),
                error = ?e,
                "Store FAIL"
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
    let payload_size = body.len();

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

    let event_count = envelope
        .items
        .iter()
        .filter(|item| item.header.item_type == "event")
        .count();
    if event_count > 1 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"Envelope has multiple events"})),
        );
    }
    let has_event = event_count == 1;
    let session_count = envelope.find_session_payloads().len();
    if !has_event && session_count == 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                serde_json::json!({"error": "Unsupported envelope items; expected event or individual session"}),
            ),
        );
    }

    let event_id = if has_event {
        let payload_id = envelope
            .find_event_payload()
            .and_then(|payload| serde_json::from_slice::<serde_json::Value>(payload).ok())
            .and_then(|event| event.get("event_id").cloned());
        // A supplied invalid ID must not become an absent ID during fallback.
        let normalize_id = |id: &str| {
            uuid::Uuid::parse_str(id)
                .map(|id| id.simple().to_string())
                .map_err(|_| ())
        };
        let header_id = envelope
            .header
            .event_id
            .as_deref()
            .map(normalize_id)
            .transpose();
        let body_id = payload_id
            .as_ref()
            .map(|id| id.as_str().ok_or(()).and_then(normalize_id))
            .transpose();
        let (header_id, body_id) = match (header_id, body_id) {
            (Ok(header_id), Ok(body_id)) => (header_id, body_id),
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "Invalid envelope or event event_id"})),
                );
            }
        };
        if header_id.is_some() && body_id.is_some() && header_id != body_id {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Envelope and event IDs differ"})),
            );
        }
        let event_id = header_id.or(body_id);
        if event_id.is_none() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Event or attachment requires a valid event_id"})),
            );
        }

        event_id
    } else {
        None
    };

    match state.ingest_use_case.execute(
        &mut conn,
        project_id,
        hash.clone(),
        compressed,
        original_size,
    ) {
        Ok(result) => {
            if result.duplicate {
                warn!(
                    project_id = %project_id,
                    payload_size,
                    duration_ms = start.elapsed().as_millis(),
                    hash = %result.hash,
                    "Envelope DUP"
                );
            } else {
                info!(
                    project_id = %project_id,
                    payload_size,
                    duration_ms = start.elapsed().as_millis(),
                    hash = %result.hash,
                    "Envelope OK"
                );
            }
            let response = if has_event {
                serde_json::json!({"id": event_id})
            } else {
                // The count acknowledges archived items, not completed projections.
                serde_json::json!({"sessions": session_count})
            };
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            let response = map_domain_error_to_response(&e);
            warn!(
                project_id = %project_id,
                payload_size,
                status = response.0.as_u16(),
                duration_ms = start.elapsed().as_millis(),
                error = ?e,
                "Envelope FAIL"
            );
            response
        }
    }
}

async fn prepare_payload(
    headers: &HeaderMap,
    body: &[u8],
    semaphore: &Semaphore,
    max_size: usize,
) -> Result<(String, Vec<u8>, Option<i32>), (StatusCode, Json<serde_json::Value>)> {
    let gzip = content_is_gzip(headers)?;
    let _permit = semaphore.try_acquire().map_err(|_| (
        StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Service overloaded, please retry"}))
    ))?;
    let body = body.to_vec();
    let result = tokio::task::spawn_blocking(move || {
        let payload = decode_body(body, gzip, max_size)?;
        let original_size = payload.len() as i32;
        let compressed = compress(&payload).map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Compression failed"}))
        ))?;
        Ok((compute_hash(&compressed), compressed, Some(original_size)))
    }).await.map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Payload worker failed"}))
    ))?;
    result
}

fn content_is_gzip(headers: &HeaderMap) -> Result<bool, (StatusCode, Json<serde_json::Value>)> {
    match headers.get("content-encoding").and_then(|value| value.to_str().ok()).unwrap_or("identity") {
        "gzip" => Ok(true),
        "identity" => Ok(false),
        _ => Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, Json(serde_json::json!({"error":"Unsupported content encoding"})))),
    }
}

fn decode_body(body: Vec<u8>, gzip: bool, max_size: usize) -> Result<Vec<u8>, (StatusCode, Json<serde_json::Value>)> {
    let payload = if gzip {
        let mut payload = Vec::new();
        GzDecoder::new(body.as_slice()).take(max_size.saturating_add(1) as u64)
            .read_to_end(&mut payload).map_err(|_| (
                StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"Invalid gzip payload"}))
            ))?;
        payload
    } else { body };
    if payload.len() > max_size || payload.len() > i32::MAX as usize {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({"error":"Uncompressed payload exceeds limit"}))));
    }
    Ok(payload)
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

#[cfg(test)]
mod protocol_tests {
    use super::*;
    #[tokio::test]
    async fn protocol_rejects_oversized_gzip_after_decoding() {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", "gzip".parse().unwrap());
        let body = compress(&vec![b'x'; 1024]).unwrap();
        let result = prepare_payload(&headers, &body, &Semaphore::new(1), 16).await;
        assert_eq!(result.unwrap_err().0, StatusCode::PAYLOAD_TOO_LARGE);
    }
    #[tokio::test]
    async fn protocol_rejects_invalid_gzip() {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", "gzip".parse().unwrap());
        assert!(prepare_payload(&headers, b"invalid", &Semaphore::new(1), 1024).await.is_err());
    }
    #[tokio::test]
    async fn protocol_rejects_unsupported_encoding() {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", "br".parse().unwrap());
        let result = prepare_payload(&headers, b"payload", &Semaphore::new(1), 1024).await;
        assert_eq!(result.unwrap_err().0, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    #[tokio::test]
    async fn protocol_store_response_returns_event_id() {
        use crate::shared::persistence::{establish_connection_pool, run_migrations, Repositories};
        let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://postgres:test@localhost/crash_cache_test".to_owned());
        let pool = establish_connection_pool(&url, 10, 30);
        run_migrations(&pool);
        let repos = Repositories::new(pool.clone());
        let project_id = repos.project.create(None, None).unwrap();
        let state = AppState {
            ingest_use_case: IngestReportUseCase::new(repos.archive, repos.queue, repos.project.clone()),
            compression_semaphore: Arc::new(Semaphore::new(1)), pool,
            project_repo: repos.project, project_cache: ProjectCache::new(Duration::from_secs(1)),
            health_cache: Arc::new(RwLock::new(HealthStats::default())), health_cache_ttl: Duration::from_secs(1),
            max_uncompressed_payload_bytes: 1024, session_repo: repos.session,
            session_status_repo: repos.session_status, session_release_repo: repos.session_release,
            session_environment_repo: repos.session_environment,
        };
        let id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let body = Bytes::from(serde_json::json!({"event_id": id, "message": "test"}).to_string());
        let response = store_report(State(state), Path(project_id), Query(SentryQueryParams { sentry_key: Some("test".to_owned()) }), HeaderMap::new(), body).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["id"], id);
    }
}
