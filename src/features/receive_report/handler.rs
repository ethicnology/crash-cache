use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::sync::OnceLock;
use tokio::sync::Semaphore;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use crate::shared::domain::DomainError;
use crate::shared::parser::Envelope;

use super::IngestReportUseCase;

const MAX_UNCOMPRESSED_SIZE: usize = 10 * 1024 * 1024;

static COMPRESSION_SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();

fn get_semaphore() -> &'static Semaphore {
    COMPRESSION_SEMAPHORE.get_or_init(|| {
        let limit = num_cpus::get() * 4;
        info!(concurrent_compressions = limit, "Compression semaphore initialized");
        Semaphore::new(limit)
    })
}

#[derive(Clone)]
pub struct AppState {
    pub ingest_use_case: IngestReportUseCase,
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
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received store report"
    );

    let (hash, compressed, original_size) = match prepare_payload(&headers, &body).await {
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
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received envelope report"
    );

    let (hash, compressed, original_size) = match prepare_payload(&headers, &body).await {
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

        let permit = get_semaphore().try_acquire();
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

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "crash-cache"
    }))
}
