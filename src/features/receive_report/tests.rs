use sha2::{Digest, Sha256};

use crate::shared::compression::GzipCompressor;
use crate::shared::domain::Project;
use crate::shared::persistence::{
    establish_connection_pool, run_migrations, ArchiveRepository, EventRepository,
    ProjectRepository, QueueRepository,
};

use super::IngestReportUseCase;

const TEST_PROJECT_ID: &str = "test-project";

fn setup_test_db() -> (
    ArchiveRepository,
    EventRepository,
    QueueRepository,
    ProjectRepository,
) {
    let pool = establish_connection_pool(":memory:");
    run_migrations(&pool);
    let project_repo = ProjectRepository::new(pool.clone());
    project_repo.save(&Project::new(TEST_PROJECT_ID.to_string())).unwrap();
    (
        ArchiveRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        QueueRepository::new(pool.clone()),
        project_repo,
    )
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

#[test]
fn test_compute_sha256_hash() {
    let payload = b"test payload";
    let mut hasher = Sha256::new();
    hasher.update(payload);
    let expected = hex::encode(hasher.finalize());

    let (archive_repo, event_repo, queue_repo, project_repo) = setup_test_db();
    let compressor = GzipCompressor::new();
    let use_case =
        IngestReportUseCase::new(archive_repo, event_repo, queue_repo, project_repo, compressor);

    let result = use_case.execute(TEST_PROJECT_ID, payload).unwrap();
    assert_eq!(result, expected);
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
fn test_ingest_stores_archive_and_event() {
    let (archive_repo, event_repo, queue_repo, project_repo) = setup_test_db();
    let compressor = GzipCompressor::new();
    let use_case = IngestReportUseCase::new(
        archive_repo.clone(),
        event_repo.clone(),
        queue_repo.clone(),
        project_repo,
        compressor,
    );

    let payload = sample_sentry_payload();
    let hash = use_case.execute(TEST_PROJECT_ID, &payload).unwrap();

    let archive = archive_repo.find_by_hash(&hash).unwrap();
    assert!(archive.is_some());
    let archive = archive.unwrap();
    assert_eq!(archive.original_size, payload.len() as i32);

    let pending_count = queue_repo.count_pending().unwrap();
    assert_eq!(pending_count, 1);
}

#[test]
fn test_deduplication_same_hash_reuses_archive() {
    let (archive_repo, event_repo, queue_repo, project_repo) = setup_test_db();
    let compressor = GzipCompressor::new();
    let use_case = IngestReportUseCase::new(
        archive_repo.clone(),
        event_repo,
        queue_repo.clone(),
        project_repo,
        compressor,
    );

    let payload = sample_sentry_payload();

    let hash1 = use_case.execute(TEST_PROJECT_ID, &payload).unwrap();
    let hash2 = use_case.execute(TEST_PROJECT_ID, &payload).unwrap();

    assert_eq!(hash1, hash2);

    let pending_count = queue_repo.count_pending().unwrap();
    assert_eq!(pending_count, 2);

    assert!(archive_repo.exists(&hash1).unwrap());
}

#[test]
fn test_different_payloads_different_hashes() {
    let (archive_repo, event_repo, queue_repo, project_repo) = setup_test_db();
    let compressor = GzipCompressor::new();
    let use_case =
        IngestReportUseCase::new(archive_repo, event_repo, queue_repo, project_repo, compressor);

    let payload1 = b"payload one";
    let payload2 = b"payload two";

    let hash1 = use_case.execute(TEST_PROJECT_ID, payload1).unwrap();
    let hash2 = use_case.execute(TEST_PROJECT_ID, payload2).unwrap();

    assert_ne!(hash1, hash2);
}

#[test]
fn test_unknown_project_returns_error() {
    let (archive_repo, event_repo, queue_repo, project_repo) = setup_test_db();
    let compressor = GzipCompressor::new();
    let use_case =
        IngestReportUseCase::new(archive_repo, event_repo, queue_repo, project_repo, compressor);

    let payload = sample_sentry_payload();
    let result = use_case.execute("unknown-project", &payload);

    assert!(result.is_err());
}
