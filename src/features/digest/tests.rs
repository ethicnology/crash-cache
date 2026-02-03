use sha2::{Digest, Sha256};

use crate::features::ingest::IngestReportUseCase;
use crate::shared::compression::GzipCompressor;
use crate::shared::domain::SentryReport;
use crate::shared::persistence::{DbPool, Repositories, establish_connection_pool, run_migrations};

use super::DigestReportUseCase;

fn test_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:test@localhost/crash_cache_test".to_string())
}

fn clean_test_db(pool: &crate::shared::persistence::DbPool) {
    use diesel::prelude::*;
    let mut conn = pool.get().expect("Failed to get connection");
    let tables = [
        "report",
        "queue_error",
        "queue",
        "session",
        "unwrap_session_status",
        "unwrap_session_release",
        "unwrap_session_environment",
        "unwrap_stacktrace",
        "unwrap_exception_message",
        "unwrap_exception_type",
        "unwrap_device_specs",
        "unwrap_user",
        "unwrap_app_build",
        "unwrap_app_version",
        "unwrap_app_name",
        "unwrap_orientation",
        "unwrap_connection_type",
        "unwrap_timezone",
        "unwrap_locale_code",
        "unwrap_chipset",
        "unwrap_model",
        "unwrap_brand",
        "unwrap_manufacturer",
        "unwrap_os_version",
        "unwrap_os_name",
        "unwrap_environment",
        "unwrap_platform",
        "issue",
        "archive",
        "project",
        "bucket_rate_limit_global",
        "bucket_rate_limit_dsn",
        "bucket_rate_limit_subnet",
        "bucket_request_latency",
    ];
    for table in tables {
        let _ = diesel::sql_query(format!("TRUNCATE TABLE {} CASCADE", table)).execute(&mut conn);
    }
}

fn setup_test_db() -> (Repositories, DbPool, i32) {
    let pool = establish_connection_pool(&test_database_url(), 10, 30);
    run_migrations(&pool);
    clean_test_db(&pool);
    let repos = Repositories::new(pool.clone());
    let project_id = repos.project.create(None, None).unwrap();
    (repos, pool, project_id)
}

fn sample_sentry_payload() -> Vec<u8> {
    r#"{
        "event_id": "abc123",
        "timestamp": "2026-01-22T10:00:00Z",
        "platform": "rust",
        "release": "my-app@1.2.3",
        "environment": "production",
        "sdk": {"name": "sentry.rust", "version": "0.32.0"},
        "exception": {
            "values": [{"type": "RuntimeError", "value": "Something went wrong"}]
        }
    }"#
    .as_bytes()
    .to_vec()
}

fn compress_and_hash(payload: &[u8]) -> (String, Vec<u8>) {
    let compressor = GzipCompressor::new();
    let compressed = compressor.compress(payload).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(&compressed);
    let hash = hex::encode(hasher.finalize());
    (hash, compressed)
}

#[test]
fn test_extract_app_version_from_release() {
    let json = r#"{"release": "my-app@1.2.3"}"#;
    let report: SentryReport = serde_json::from_str(json).unwrap();
    assert_eq!(report.extract_app_version(), Some("1.2.3".to_string()));
}

#[test]
fn test_extract_app_version_from_context() {
    let json = r#"{"contexts": {"app": {"app_version": "2.0.0"}}}"#;
    let report: SentryReport = serde_json::from_str(json).unwrap();
    assert_eq!(report.extract_app_version(), Some("2.0.0".to_string()));
}

#[test]
fn test_extract_error_info() {
    let json = r#"{"exception": {"values": [{"type": "ValueError", "value": "Invalid input"}]}}"#;
    let report: SentryReport = serde_json::from_str(json).unwrap();
    let (error_type, error_message) = report.extract_error_info();
    assert_eq!(error_type, Some("ValueError".to_string()));
    assert_eq!(error_message, Some("Invalid input".to_string()));
}

#[test]
fn test_extract_sdk_info() {
    let json = r#"{"sdk": {"name": "sentry.python", "version": "1.5.0"}}"#;
    let report: SentryReport = serde_json::from_str(json).unwrap();
    let (sdk_name, sdk_version) = report.extract_sdk_info();
    assert_eq!(sdk_name, Some("sentry.python".to_string()));
    assert_eq!(sdk_version, Some("1.5.0".to_string()));
}

#[test]
fn test_process_extracts_and_stores_report() {
    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();
    let queue_repo = repos.queue.clone();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );

    let process_use_case = DigestReportUseCase::new(repos.clone(), pool, compressor);

    let payload = sample_sentry_payload();
    let (hash, compressed) = compress_and_hash(&payload);
    ingest_use_case
        .execute(project_id, hash, compressed, None)
        .unwrap();

    let processed = process_use_case.process_batch(10).unwrap();
    assert_eq!(processed, 1);

    let pending = queue_repo.count_pending().unwrap();
    assert_eq!(pending, 0);
}

#[test]
fn test_process_batch_returns_zero_when_empty() {
    let (repos, pool, _project_id) = setup_test_db();
    let compressor = GzipCompressor::new();

    let process_use_case = DigestReportUseCase::new(repos, pool, compressor);

    let processed = process_use_case.process_batch(10).unwrap();
    assert_eq!(processed, 0);
}

#[test]
fn test_process_multiple_events() {
    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();
    let queue_repo = repos.queue.clone();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );

    let process_use_case = DigestReportUseCase::new(repos, pool, compressor);

    let payload1 = r#"{"event_id": "e1", "release": "app@1.0.0", "platform": "python"}"#.as_bytes();
    let payload2 = r#"{"event_id": "e2", "release": "app@2.0.0", "platform": "rust"}"#.as_bytes();
    let payload3 = r#"{"event_id": "e3", "release": "app@3.0.0", "platform": "go"}"#.as_bytes();

    let (h1, c1) = compress_and_hash(payload1);
    let (h2, c2) = compress_and_hash(payload2);
    let (h3, c3) = compress_and_hash(payload3);

    ingest_use_case.execute(project_id, h1, c1, None).unwrap();
    ingest_use_case.execute(project_id, h2, c2, None).unwrap();
    ingest_use_case.execute(project_id, h3, c3, None).unwrap();

    assert_eq!(queue_repo.count_pending().unwrap(), 3);

    let processed = process_use_case.process_batch(10).unwrap();
    assert_eq!(processed, 3);

    assert_eq!(queue_repo.count_pending().unwrap(), 0);
}
