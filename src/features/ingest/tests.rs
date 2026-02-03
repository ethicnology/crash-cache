use sha2::{Digest, Sha256};

use crate::shared::compression::GzipCompressor;
use crate::shared::persistence::{Repositories, establish_connection_pool, run_migrations};

use super::IngestReportUseCase;

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

fn setup_test_db() -> (Repositories, i32) {
    let pool = establish_connection_pool(&test_database_url());
    run_migrations(&pool);
    clean_test_db(&pool);
    let repos = Repositories::new(pool);
    let project_id = repos.project.create(None, None).unwrap();
    (repos, project_id)
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
    let hash = compute_hash(&compressed);
    (hash, compressed)
}

fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[test]
fn test_hash_of_compressed_payload() {
    let payload = b"test payload";
    let (hash, compressed) = compress_and_hash(payload);

    let expected_hash = compute_hash(&compressed);
    assert_eq!(hash, expected_hash);
}

#[test]
fn test_gzip_compression_roundtrip() {
    let compressor = GzipCompressor::new();
    let original = sample_sentry_payload();

    let compressed = compressor.compress(&original).unwrap();
    assert!(compressed.len() < original.len());

    let decompressed = compressor.decompress(&compressed).unwrap();
    assert_eq!(decompressed, original);
}

#[test]
fn test_ingest_stores_archive() {
    let (repos, project_id) = setup_test_db();
    let archive_repo = repos.archive.clone();
    let queue_repo = repos.queue.clone();
    let use_case = IngestReportUseCase::new(repos.archive, repos.queue, repos.project);

    let payload = sample_sentry_payload();
    let original_size = payload.len() as i32;
    let (hash, compressed) = compress_and_hash(&payload);

    let result_hash = use_case
        .execute(project_id, hash.clone(), compressed, Some(original_size))
        .unwrap();

    assert_eq!(result_hash, hash);

    let archive = archive_repo.find_by_hash(&hash).unwrap();
    assert!(archive.is_some());
    assert_eq!(archive.unwrap().original_size, Some(original_size));

    let pending_count = queue_repo.count_pending().unwrap();
    assert_eq!(pending_count, 1);
}

#[test]
fn test_deduplication_same_hash_reuses_archive() {
    let (repos, project_id) = setup_test_db();
    let archive_repo = repos.archive.clone();
    let queue_repo = repos.queue.clone();
    let use_case = IngestReportUseCase::new(repos.archive, repos.queue, repos.project);

    let payload = sample_sentry_payload();
    let (hash, compressed) = compress_and_hash(&payload);

    let hash1 = use_case
        .execute(project_id, hash.clone(), compressed.clone(), None)
        .unwrap();
    let hash2 = use_case
        .execute(project_id, hash.clone(), compressed, None)
        .unwrap();

    assert_eq!(hash1, hash2);

    let pending_count = queue_repo.count_pending().unwrap();
    assert_eq!(pending_count, 1);

    assert!(archive_repo.exists(&hash1).unwrap());
}

#[test]
fn test_different_payloads_different_hashes() {
    let (repos, project_id) = setup_test_db();
    let use_case = IngestReportUseCase::new(repos.archive, repos.queue, repos.project);

    let (hash1, compressed1) = compress_and_hash(b"payload one");
    let (hash2, compressed2) = compress_and_hash(b"payload two");

    let result1 = use_case
        .execute(project_id, hash1.clone(), compressed1, None)
        .unwrap();
    let result2 = use_case
        .execute(project_id, hash2.clone(), compressed2, None)
        .unwrap();

    assert_ne!(result1, result2);
}

#[test]
fn test_unknown_project_returns_error() {
    let (repos, _project_id) = setup_test_db();
    let use_case = IngestReportUseCase::new(repos.archive, repos.queue, repos.project);

    let (hash, compressed) = compress_and_hash(&sample_sentry_payload());
    let result = use_case.execute(999, hash, compressed, None);

    assert!(result.is_err());
}
