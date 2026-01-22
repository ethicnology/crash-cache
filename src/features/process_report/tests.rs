use crate::features::receive_report::IngestReportUseCase;
use crate::shared::compression::GzipCompressor;
use crate::shared::domain::SentryReport;
use crate::shared::persistence::{
    establish_connection_pool, run_migrations, ArchiveRepository, EventRepository, QueueRepository,
    ReportMetadataRepository,
};

use super::ProcessReportUseCase;

fn setup_test_db() -> (
    ArchiveRepository,
    EventRepository,
    QueueRepository,
    ReportMetadataRepository,
) {
    let pool = establish_connection_pool(":memory:");
    run_migrations(&pool);
    (
        ArchiveRepository::new(pool.clone()),
        EventRepository::new(pool.clone()),
        QueueRepository::new(pool.clone()),
        ReportMetadataRepository::new(pool),
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
fn test_process_extracts_metadata() {
    let (archive_repo, event_repo, queue_repo, metadata_repo) = setup_test_db();
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        archive_repo.clone(),
        event_repo.clone(),
        queue_repo.clone(),
        compressor.clone(),
    );

    let process_use_case = ProcessReportUseCase::new(
        archive_repo,
        event_repo.clone(),
        queue_repo.clone(),
        metadata_repo.clone(),
        compressor,
    );

    let payload = sample_sentry_payload();
    ingest_use_case.execute(&payload).unwrap();

    let processed = process_use_case.process_batch(10).unwrap();
    assert_eq!(processed, 1);

    let pending = queue_repo.count_pending().unwrap();
    assert_eq!(pending, 0);

    let event = event_repo.find_by_id(1).unwrap().unwrap();
    assert!(event.processed);

    let metadata = metadata_repo.find_by_event_id(1).unwrap().unwrap();
    assert_eq!(metadata.app_version, Some("1.2.3".to_string()));
    assert_eq!(metadata.platform, Some("rust".to_string()));
    assert_eq!(metadata.environment, Some("production".to_string()));
    assert_eq!(metadata.error_type, Some("RuntimeError".to_string()));
    assert_eq!(
        metadata.error_message,
        Some("Something went wrong".to_string())
    );
    assert_eq!(metadata.sdk_name, Some("sentry.rust".to_string()));
    assert_eq!(metadata.sdk_version, Some("0.32.0".to_string()));
}

#[test]
fn test_process_batch_returns_zero_when_empty() {
    let (archive_repo, event_repo, queue_repo, metadata_repo) = setup_test_db();
    let compressor = GzipCompressor::new();

    let process_use_case = ProcessReportUseCase::new(
        archive_repo,
        event_repo,
        queue_repo,
        metadata_repo,
        compressor,
    );

    let processed = process_use_case.process_batch(10).unwrap();
    assert_eq!(processed, 0);
}

#[test]
fn test_process_multiple_events() {
    let (archive_repo, event_repo, queue_repo, metadata_repo) = setup_test_db();
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        archive_repo.clone(),
        event_repo.clone(),
        queue_repo.clone(),
        compressor.clone(),
    );

    let process_use_case = ProcessReportUseCase::new(
        archive_repo,
        event_repo,
        queue_repo.clone(),
        metadata_repo,
        compressor,
    );

    let payload1 = r#"{"release": "app@1.0.0", "platform": "python"}"#.as_bytes();
    let payload2 = r#"{"release": "app@2.0.0", "platform": "rust"}"#.as_bytes();
    let payload3 = r#"{"release": "app@3.0.0", "platform": "go"}"#.as_bytes();

    ingest_use_case.execute(payload1).unwrap();
    ingest_use_case.execute(payload2).unwrap();
    ingest_use_case.execute(payload3).unwrap();

    assert_eq!(queue_repo.count_pending().unwrap(), 3);

    let processed = process_use_case.process_batch(10).unwrap();
    assert_eq!(processed, 3);

    assert_eq!(queue_repo.count_pending().unwrap(), 0);
}
