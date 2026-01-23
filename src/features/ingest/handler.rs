use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use diesel::prelude::*;
use diesel::sql_query;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use crate::shared::domain::DomainError;
use crate::shared::parser::Envelope;
use crate::shared::persistence::{ProjectRepository, SqlitePool};

use super::IngestReportUseCase;

const MAX_UNCOMPRESSED_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct SentryQueryParams {
    pub sentry_key: Option<String>,
}

#[derive(Clone, Default)]
pub struct HealthStats {
    archives: i64,
    reports: i64,
    queue: i64,
    orphaned: i64,
    updated_at: Option<Instant>,
}

#[derive(Clone)]
pub struct AppState {
    pub ingest_use_case: IngestReportUseCase,
    pub compression_semaphore: Arc<Semaphore>,
    pub pool: SqlitePool,
    pub project_repo: ProjectRepository,
    pub health_cache: Arc<RwLock<HealthStats>>,
    pub health_cache_ttl: Duration,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/{project_id}/store/", post(store_report))
        .route("/api/{project_id}/store", post(store_report))
        .route("/api/{project_id}/envelope/", post(envelope_report))
        .route("/api/{project_id}/envelope", post(envelope_report))
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn store_report(
    State(state): State<AppState>,
    Path(project_id): Path<i32>,
    Query(query): Query<SentryQueryParams>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received store report"
    );

    // Validate sentry_key
    let sentry_key = extract_sentry_key(&headers, &query);
    if let Err(response) = validate_project_key(&state.project_repo, project_id, sentry_key) {
        return response;
    }

    let (hash, compressed, original_size) = match prepare_payload(&headers, &body, &state.compression_semaphore).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    match state.ingest_use_case.execute(project_id, hash.clone(), compressed, original_size) {
        Ok(_) => {
            info!(hash = %hash, "Report stored successfully");
            (StatusCode::OK, Json(serde_json::json!({"id": hash})))
        }
        Err(DomainError::ProjectNotFound(pid)) => {
            warn!(project_id = %pid, "Project not found, dropping report");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            error!(error = %e, "Failed to store report");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
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
    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received envelope report"
    );

    // Validate sentry_key
    let sentry_key = extract_sentry_key(&headers, &query);
    if let Err(response) = validate_project_key(&state.project_repo, project_id, sentry_key) {
        return response;
    }

    let (hash, compressed, original_size) = match prepare_payload(&headers, &body, &state.compression_semaphore).await {
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

    info!(
        "Envelope has {} items: {:?}",
        envelope.items.len(),
        envelope.items.iter().map(|i| (&i.header.item_type, i.payload.len())).collect::<Vec<_>>()
    );

    if envelope.find_event_payload().is_none() {
        warn!("No event found in envelope");
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No event in envelope"})),
        );
    }

    match state.ingest_use_case.execute(project_id, hash.clone(), compressed, original_size) {
        Ok(_) => {
            info!(hash = %hash, "Envelope report stored successfully");
            (StatusCode::OK, Json(serde_json::json!({"id": hash})))
        }
        Err(DomainError::ProjectNotFound(pid)) => {
            warn!(project_id = %pid, "Project not found, dropping report");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            error!(error = %e, "Failed to store envelope report");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

async fn prepare_payload(
    headers: &HeaderMap,
    body: &[u8],
    semaphore: &Semaphore,
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
        if body.len() > MAX_UNCOMPRESSED_SIZE {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({
                    "error": format!("Payload too large: {} bytes (max {})", body.len(), MAX_UNCOMPRESSED_SIZE)
                })),
            ));
        }

        let permit = semaphore.try_acquire();
        if permit.is_err() {
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

    match project_repo.validate_key(project_id, &key) {
        Ok(true) => Ok(()),
        Ok(false) => {
            warn!(project_id = %project_id, "Invalid public key");
            Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid public key"})),
            ))
        }
        Err(DomainError::ProjectNotFound(pid)) => {
            warn!(project_id = %pid, "Project not found");
            Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            ))
        }
        Err(e) => {
            error!(error = %e, "Failed to validate key");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            ))
        }
    }
}

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let stats = get_cached_stats(&state);
    
    Json(serde_json::json!({
        "status": "ok",
        "service": "crash-cache",
        "stats": {
            "ingested": stats.archives,
            "digested": stats.reports,
            "queued": stats.queue,
            "orphaned": stats.orphaned
        }
    }))
}

fn get_cached_stats(state: &AppState) -> HealthStats {
    {
        let cache = state.health_cache.read().unwrap();
        if let Some(updated_at) = cache.updated_at {
            if updated_at.elapsed() < state.health_cache_ttl {
                return cache.clone();
            }
        }
    }

    let mut conn = match state.pool.get() {
        Ok(c) => c,
        Err(_) => return HealthStats::default(),
    };

    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        c: i64,
    }

    let archives = sql_query("SELECT COUNT(*) as c FROM archive")
        .get_result::<Count>(&mut conn)
        .map(|r| r.c)
        .unwrap_or(0);

    let reports = sql_query("SELECT COUNT(*) as c FROM report")
        .get_result::<Count>(&mut conn)
        .map(|r| r.c)
        .unwrap_or(0);

    let queue = sql_query("SELECT COUNT(*) as c FROM processing_queue")
        .get_result::<Count>(&mut conn)
        .map(|r| r.c)
        .unwrap_or(0);

    let orphaned = sql_query(
        "SELECT COUNT(*) as c FROM archive a
         WHERE NOT EXISTS (SELECT 1 FROM report r WHERE r.archive_hash = a.hash)
         AND NOT EXISTS (SELECT 1 FROM processing_queue q WHERE q.archive_hash = a.hash)"
    )
        .get_result::<Count>(&mut conn)
        .map(|r| r.c)
        .unwrap_or(0);

    let new_stats = HealthStats {
        archives,
        reports,
        queue,
        orphaned,
        updated_at: Some(Instant::now()),
    };

    if let Ok(mut cache) = state.health_cache.write() {
        *cache = new_stats.clone();
    }

    new_stats
}
