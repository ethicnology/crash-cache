use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use tracing::{error, info};

use super::IngestCrashUseCase;

#[derive(Clone)]
pub struct AppState {
    pub ingest_use_case: IngestCrashUseCase,
}

#[derive(Serialize)]
struct StoreResponse {
    id: String,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/{project_id}/store/", post(store_crash))
        .route("/api/{project_id}/store", post(store_crash))
        .route("/health", get(health_check))
        .with_state(state)
}

async fn store_crash(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    body: Bytes,
) -> impl IntoResponse {
    info!(
        project_id = %project_id,
        payload_size = body.len(),
        "Received crash report"
    );

    match state.ingest_use_case.execute(&body) {
        Ok(hash) => {
            info!(hash = %hash, "Crash report stored successfully");
            (StatusCode::OK, Json(StoreResponse { id: hash }))
        }
        Err(e) => {
            error!(error = %e, "Failed to store crash report");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(StoreResponse { id: String::new() }),
            )
        }
    }
}

async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}
