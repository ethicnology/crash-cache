use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use flate2::read::GzDecoder;
use std::io::Read;
use tracing::{error, info, warn};

use crate::shared::domain::DomainError;
use crate::shared::parser::Envelope;

use super::IngestReportUseCase;

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
        .with_state(state)
}

async fn store_report(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received store report"
    );

    let payload = match decompress_if_needed(&headers, &body) {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "Failed to decompress payload");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    };

    match state.ingest_use_case.execute(&project_id, &payload) {
        Ok(hash) => {
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
    Path(project_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received envelope report"
    );

    let raw_data = match decompress_if_needed(&headers, &body) {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "Failed to decompress envelope");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    };

    let envelope = match Envelope::parse(&raw_data) {
        Some(e) => e,
        None => {
            warn!("Failed to parse envelope format");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid envelope format"})),
            );
        }
    };

    let event_payload = match envelope.find_event_payload() {
        Some(p) => p,
        None => {
            warn!("No event found in envelope");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "No event in envelope"})),
            );
        }
    };

    match state.ingest_use_case.execute(&project_id, event_payload) {
        Ok(hash) => {
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

fn decompress_if_needed(headers: &HeaderMap, body: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let content_encoding = headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_encoding.contains("gzip") {
        let mut decoder = GzDecoder::new(body);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    } else {
        Ok(body.to_vec())
    }
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "crash-cache"
    }))
}
