use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use crash_cache::{
    features::{
        digest::DigestReportUseCase,
        ingest::{AppState, HealthStats, IngestReportUseCase, ProjectCache, create_api_router},
    },
    shared::{
        compression::GzipCompressor,
        persistence::{Repositories, establish_connection_pool, run_migrations},
    },
};
use serde_json::json;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use tower::ServiceExt;

async fn submit(
    header: serde_json::Value,
    event: serde_json::Value,
) -> (axum::response::Response, u32, i64) {
    let url = std::env::var("DATABASE_URL").expect("isolated DATABASE_URL required");
    assert!(url.contains("crash_cache_test"));
    let pool = establish_connection_pool(&url, 4, 5);
    run_migrations(&pool);
    let repos = Repositories::new(pool.clone());
    let project = repos.project.create(None, None).unwrap();
    let state = AppState {
        ingest_use_case: IngestReportUseCase::new(
            repos.archive.clone(),
            repos.queue.clone(),
            repos.project.clone(),
        ),
        compression_semaphore: Arc::new(Semaphore::new(1)),
        pool: pool.clone(),
        project_repo: repos.project.clone(),
        project_cache: ProjectCache::new(Duration::from_secs(1)),
        health_cache: Arc::new(RwLock::new(HealthStats::default())),
        health_cache_ttl: Duration::from_secs(1),
        max_uncompressed_payload_bytes: 4096,
        session_repo: repos.session.clone(),
        session_status_repo: repos.session_status.clone(),
        session_release_repo: repos.session_release.clone(),
        session_environment_repo: repos.session_environment.clone(),
    };
    let event = serde_json::to_vec(&event).unwrap();
    let mut bytes = format!(
        "{}\n{}\n",
        header,
        json!({"type":"event","length":event.len()})
    )
    .into_bytes();
    bytes.extend_from_slice(&event);
    let response = create_api_router(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/{project}/envelope/?sentry_key=test"))
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    // Follow the admitted request through the real digest before reporting the admission error.
    let processed = DigestReportUseCase::new(repos.clone(), pool.clone(), GzipCompressor::new())
        .process_batch(100)
        .unwrap();
    let reports = repos.report.count_by_project(project).unwrap();
    (response, processed, reports)
}

async fn check(header_id: &str, body_id: &str) {
    let (response, processed, reports) = submit(
        json!({"event_id": header_id}),
        json!({"event_id": body_id, "message": "synthetic identity check"}),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "malformed supplied IDs must be rejected before acknowledgement; digest processed={processed}, project reports={reports}"
    );
    assert_eq!(reports, 0);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn a_valid_header_must_not_hide_a_malformed_body_id() {
    check("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "not-a-uuid").await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn a_valid_body_must_not_hide_a_malformed_header_id() {
    check("not-a-uuid", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").await;
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn valid_identity_fallback_and_normalization_still_persist_reports() {
    let id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    for (header, event) in [
        (json!({"event_id": id}), json!({"message": "header only"})),
        (json!({}), json!({"event_id": id, "message": "body only"})),
        (
            json!({"event_id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"}),
            json!({"event_id": id, "message": "equivalent IDs"}),
        ),
    ] {
        let (response, processed, reports) = submit(header, event).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"],
            id
        );
        assert_eq!(processed, 1);
        assert_eq!(reports, 1);
    }
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn present_non_string_body_ids_cannot_fall_back_to_the_header() {
    for invalid in [json!(null), json!(42), json!({}), json!([])] {
        let (response, _, reports) = submit(
            json!({"event_id": "cccccccccccccccccccccccccccccccc"}),
            json!({"event_id": invalid, "message": "invalid ID type"}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(reports, 0);
    }
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn missing_or_conflicting_event_ids_are_rejected() {
    for (header, event) in [
        (json!({}), json!({"message": "missing identity"})),
        (
            json!({"event_id": "dddddddddddddddddddddddddddddddd"}),
            json!({"event_id": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", "message": "conflicting identity"}),
        ),
    ] {
        let (response, _, reports) = submit(header, event).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(reports, 0);
    }
}
