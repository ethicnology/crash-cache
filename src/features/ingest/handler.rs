use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, FromRequest, Multipart, Path, Query, State},
    http::{HeaderMap, Request, StatusCode},
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
        .route("/api/{project_id}/minidump/", post(minidump_report))
        .route("/api/{project_id}/minidump", post(minidump_report))
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

async fn minidump_report(
    State(state): State<AppState>,
    Path(project_id): Path<i32>,
    Query(query): Query<SentryQueryParams>,
    mut headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // Crashpad compresses the entire multipart body. Decode before asking the
    // multipart extractor to parse boundaries, retaining both wire and decoded limits.
    let gzip = match content_is_gzip(&headers) {
        Ok(value) => value,
        Err(response) => return response.into_response(),
    };
    let permit = match state.compression_semaphore.try_acquire() {
        Ok(permit) => permit,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"Service overloaded, please retry"}))).into_response(),
    };
    let max_size = state.max_uncompressed_payload_bytes;
    let decoded = tokio::task::spawn_blocking(move || decode_body(body.to_vec(), gzip, max_size)).await;
    drop(permit);
    let decoded = match decoded {
        Ok(Ok(body)) => body,
        Ok(Err(response)) => return response.into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"Payload worker failed"}))).into_response(),
    };
    headers.remove("content-encoding");
    headers.remove("content-length");
    let mut request = Request::new(Body::from(decoded));
    *request.headers_mut() = headers.clone();
    DefaultBodyLimit::max(max_size).apply(&mut request);
    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(rejection) => return rejection.into_response(),
    };
    let mut attachments: Vec<(String, Option<String>, String, Vec<u8>)> = Vec::new();
    let mut dump: Option<Vec<u8>> = None;
    let mut event: Option<serde_json::Value> = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"Invalid multipart body"}))).into_response(),
        };
        let name = field.name().unwrap_or_default().to_owned();
        let filename = field.file_name().map(str::to_owned);
        let content_type = field.content_type().unwrap_or("application/octet-stream").to_owned();
        let bytes = match field.bytes().await {
            Ok(bytes) if bytes.len() <= state.max_uncompressed_payload_bytes => bytes,
            _ => return (StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({"error":"Invalid or oversized minidump field"}))).into_response(),
        };
        match name.as_str() {
            "upload_file_minidump" if dump.is_none() => dump = Some(bytes.to_vec()),
            "sentry" | "__sentry-event" => {
                if event.is_some() {
                    return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"Multiple event metadata fields"}))).into_response();
                }
                let parsed = if name == "__sentry-event" {
                    decode_crashpad_event(&bytes)
                } else {
                    serde_json::from_slice::<serde_json::Value>(&bytes).ok().filter(serde_json::Value::is_object)
                };
                match parsed {
                    Some(value) => event = Some(value),
                    None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"Invalid sentry metadata"}))).into_response(),
                }
            }
            "upload_file_minidump" => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"Multiple minidumps"}))).into_response(),
            _ => attachments.push((name, filename, content_type, bytes.to_vec())),
        }
    }
    let Some(dump) = dump else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"Missing minidump"}))).into_response();
    };
    let mut event = event.unwrap_or_else(|| serde_json::json!({}));
    let id = event.get("event_id").and_then(serde_json::Value::as_str)
        .and_then(|id| uuid::Uuid::parse_str(id).ok()).unwrap_or_else(uuid::Uuid::new_v4).simple().to_string();
    event["event_id"] = serde_json::json!(id);
    let event_bytes = serde_json::to_vec(&event).unwrap_or_default();
    let mut envelope = format!("{}\n{}\n", serde_json::json!({"event_id":id}), serde_json::json!({"type":"event","length":event_bytes.len()})).into_bytes();
    envelope.extend_from_slice(&event_bytes);
    envelope.extend_from_slice(format!("\n{}\n", serde_json::json!({"type":"attachment","attachment_type":"event.minidump","length":dump.len(),"filename":"crash.dmp"})).as_bytes());
    envelope.extend_from_slice(&dump);
    for (name, filename, content_type, bytes) in attachments {
        envelope.extend_from_slice(format!("\n{}\n", serde_json::json!({
            "type":"attachment", "attachment_type":"event.attachment",
            "length":bytes.len(), "filename":filename.unwrap_or(name), "content_type":content_type
        })).as_bytes());
        envelope.extend_from_slice(&bytes);
    }
    headers.remove("content-encoding");
    envelope_report(State(state), Path(project_id), Query(query), headers, Bytes::from(envelope)).await.into_response()
}

/// Sentry Native's Crashpad attachment contains one MessagePack scope object.
fn decode_crashpad_event(bytes: &[u8]) -> Option<serde_json::Value> {
    let mut decoder = rmp_serde::Deserializer::new(std::io::Cursor::new(bytes));
    decoder.set_max_depth(64);
    let event = serde_json::Value::deserialize(&mut decoder).ok()?;
    if !event.is_object() || decoder.position() != bytes.len() as u64 {
        return None;
    }
    Some(event)
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

    let event_count = envelope.items.iter().filter(|item| item.header.item_type == "event").count();
    let minidump_count = envelope.items.iter().filter(|item| item.header.item_type == "attachment"
        && item.header.extra.get("attachment_type").and_then(serde_json::Value::as_str) == Some("event.minidump")).count();
    if event_count > 1 || minidump_count > 1 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Envelope has multiple events or minidumps"})));
    }
    let has_event = event_count == 1 || minidump_count == 1;

    let has_logs = envelope.items.iter().any(|item| item.header.item_type == "log");
    let has_attachments = envelope.items.iter().any(|item| item.header.item_type == "attachment");
    if let Err(error) = crate::shared::parser::sentry_log::parse_logs(&envelope) {
        return map_domain_error_to_response(&error);
    }
    if !has_event && !has_logs && !has_attachments {
        let payloads = envelope.find_session_payloads();
        if payloads.is_empty() {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": "Unsupported envelope items; expected event or individual session"})),
            );
        }
        let sessions: Option<Vec<SentrySession>> = payloads.into_iter().map(SentrySession::parse).collect();
        let Some(sessions) = sessions else {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid session payload"})),
            );
        };
        let result: Result<usize, DomainError> = conn.transaction(|conn| {
            for session in &sessions {
                store_session(&state, conn, project_id, session)?;
            }
            Ok(sessions.len())
        });
        return match result {
            Ok(count) => (StatusCode::OK, Json(serde_json::json!({"sessions": count}))),
            Err(error) => map_domain_error_to_response(&error),
        };
    }

    let payload_id = envelope.find_event_payload()
        .and_then(|payload| serde_json::from_slice::<serde_json::Value>(payload).ok())
        .and_then(|event| event.get("event_id").and_then(|id| id.as_str()).map(str::to_owned));
    let normalize_id = |id: &str| uuid::Uuid::parse_str(id).ok().map(|id| id.simple().to_string());
    let header_id = envelope.header.event_id.as_deref().and_then(normalize_id);
    let body_id = payload_id.as_deref().and_then(normalize_id);
    if header_id.is_some() && body_id.is_some() && header_id != body_id {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Envelope and event IDs differ"})));
    }
    let event_id = header_id.or(body_id);
    if (has_event || has_attachments) && event_id.is_none() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Event or attachment requires a valid event_id"})));
    }

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
                "Envelope FAIL"
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
        distinct_id: session.distinct_id_hash(),
        sequence: session.sequence_decimal(),
        duration: session.duration,
        abnormal_mechanism: session.abnormal_mechanism.clone(),
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

    async fn multipart_response(dump_size: usize, gzip_body: bool) -> StatusCode {
        multipart_response_with_metadata(dump_size, gzip_body, None).await
    }

    async fn multipart_response_with_metadata(dump_size: usize, gzip_body: bool, metadata: Option<serde_json::Value>) -> StatusCode {
        use crate::shared::persistence::{establish_connection_pool, run_migrations, Repositories};
        use axum::{body::Body, extract::DefaultBodyLimit, http::Request};
        use tower::ServiceExt;
        let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://postgres:test@localhost/crash_cache_test".to_owned());
        assert!(url.contains("crash_cache_test"), "Multipart regression must use the test database");
        let pool = establish_connection_pool(&url, 10, 30);
        run_migrations(&pool);
        let repos = Repositories::new(pool.clone());
        let project_id = repos.project.create(None, None).unwrap();
        let state = AppState {
            ingest_use_case: IngestReportUseCase::new(repos.archive, repos.queue, repos.project.clone()),
            compression_semaphore: Arc::new(Semaphore::new(1)), pool,
            project_repo: repos.project, project_cache: ProjectCache::new(Duration::from_secs(1)),
            health_cache: Arc::new(RwLock::new(HealthStats::default())), health_cache_ttl: Duration::from_secs(1),
            max_uncompressed_payload_bytes: 4 * 1024 * 1024, session_repo: repos.session,
            session_status_repo: repos.session_status, session_release_repo: repos.session_release,
            session_environment_repo: repos.session_environment,
        };
        let mut body = b"--test-boundary\r\nContent-Disposition: form-data; name=\"sentry\"\r\n\r\n{\"event_id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}\r\n--test-boundary\r\nContent-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"synthetic.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
        if let Some(metadata) = &metadata {
            body = b"--test-boundary\r\nContent-Disposition: form-data; name=\"__sentry-event\"; filename=\"__sentry-event\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
            body.extend_from_slice(&rmp_serde::to_vec_named(metadata).unwrap());
            body.extend_from_slice(b"\r\n--test-boundary\r\nContent-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"synthetic.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\n");
        }
        body.extend(std::iter::repeat_n(b'x', dump_size));
        if metadata.is_some() {
            body.extend_from_slice(b"\r\n--test-boundary\r\nContent-Disposition: form-data; name=\"__sentry-breadcrumb1\"; filename=\"__sentry-breadcrumb1\"\r\nContent-Type: application/octet-stream\r\n\r\n\x00\xffopaque\nbytes");
        }
        body.extend_from_slice(b"\r\n--test-boundary--\r\n");
        let body = if gzip_body { compress(&body).unwrap() } else { body };
        let mut request = Request::builder().method("POST")
            .uri(format!("/api/{project_id}/minidump/?sentry_key=test"))
            .header("content-type", "multipart/form-data; boundary=test-boundary");
        if gzip_body { request = request.header("content-encoding", "gzip"); }
        let pool = state.pool.clone();
        let router = create_api_router(state).layer(DefaultBodyLimit::max(8 * 1024 * 1024));
        let response = router.oneshot(request.body(Body::from(body)).unwrap()).await.unwrap();
        let status = response.status();
        if let Some(expected) = metadata {
            let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
            let acknowledgement: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(acknowledgement["id"], expected["event_id"]);
            use crate::shared::persistence::db::{schema::archive, models::ArchiveModel};
            let stored = archive::table.filter(archive::project_id.eq(project_id)).first::<ArchiveModel>(&mut pool.get().unwrap()).unwrap();
            let raw = decompress(&stored.compressed_payload).unwrap();
            let envelope = Envelope::parse(&raw).unwrap();
            let event: serde_json::Value = serde_json::from_slice(envelope.find_event_payload().unwrap()).unwrap();
            assert_eq!(event["user"], expected["user"]);
            assert_eq!(event["tags"], expected["tags"]);
            assert_eq!(event["contexts"], expected["contexts"]);
            let attachment = envelope.items.iter().find(|item| item.header.extra.get("filename").and_then(serde_json::Value::as_str) == Some("__sentry-breadcrumb1")).expect("opaque Crashpad attachment must survive normalization");
            assert_eq!(attachment.payload, b"\x00\xffopaque\nbytes");
            assert_eq!(attachment.header.content_type.as_deref(), Some("application/octet-stream"));
        }
        status
    }

    #[tokio::test]
    async fn protocol_multipart_larger_than_default_limit_is_accepted() {
        assert_eq!(multipart_response(3 * 1024 * 1024, false).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn protocol_crashpad_gzip_multipart_is_accepted() {
        assert_eq!(multipart_response(3 * 1024 * 1024, true).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn protocol_crashpad_gzip_multipart_obeys_decompressed_limit() {
        assert_eq!(multipart_response(5 * 1024 * 1024, true).await, StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn protocol_crashpad_preserves_msgpack_scope_metadata() {
        let metadata = serde_json::json!({
            "event_id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "user":{"id":"synthetic-installation"},
            "tags":{"layer":"native","operation_id":"synthetic-operation"},
            "contexts":{"os":{"name":"Linux"},"device":{"arch":"x86_64"}}
        });
        assert_eq!(multipart_response_with_metadata(32, true, Some(metadata)).await, StatusCode::OK);
    }

    #[test]
    fn crashpad_metadata_rejects_invalid_or_ambiguous_payloads() {
        assert!(decode_crashpad_event(b"not messagepack").is_none());
        assert!(decode_crashpad_event(&rmp_serde::to_vec(&vec![1, 2]).unwrap()).is_none());
        let mut payload = rmp_serde::to_vec_named(&serde_json::json!({"event_id":"synthetic"})).unwrap();
        payload.push(0xc0);
        assert!(decode_crashpad_event(&payload).is_none());
        let mut deep = serde_json::json!({});
        for _ in 0..70 { deep = serde_json::json!({"nested":deep}); }
        assert!(decode_crashpad_event(&rmp_serde::to_vec_named(&deep).unwrap()).is_none());
    }

}
