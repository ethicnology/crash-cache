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
        "report_breadcrumb",
        "report_tag",
        "report_context",
        "report",
        "queue_error",
        "queue",
        "session",
        "unwrap_session_status",
        "unwrap_session_release",
        "unwrap_session_environment",
        "unwrap_breadcrumb",
        "unwrap_breadcrumb_category",
        "unwrap_breadcrumb_type",
        "unwrap_breadcrumb_level",
        "unwrap_stacktrace",
        "unwrap_exception_message",
        "unwrap_exception_type",
        "unwrap_tag_key",
        "unwrap_tag_value",
        "unwrap_context_key",
        "unwrap_context_value",
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

    let process_use_case = DigestReportUseCase::new(repos.clone(), pool.clone(), compressor);

    let payload = sample_sentry_payload();
    let (hash, compressed) = compress_and_hash(&payload);
    let mut conn = pool.get().unwrap();
    ingest_use_case
        .execute(&mut conn, project_id, hash, compressed, None)
        .unwrap();

    let processed = process_use_case.process_batch(10).unwrap();
    assert_eq!(processed, 1);

    let pending = queue_repo.count_pending(&mut conn).unwrap();
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
fn test_process_extracts_tags_and_contexts() {
    use diesel::prelude::*;
    use crate::shared::persistence::db::schema::{report_tag, report_context};

    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );
    let process_use_case = DigestReportUseCase::new(repos.clone(), pool.clone(), compressor);

    // Event with rich tags + custom contexts (mirrors BULL's Report.dart shape).
    // Built-in contexts (device, os, app, ...) are extracted via dedicated
    // unwrap_* tables; only the unknown ones flow into report_context.
    let payload = r#"{
        "event_id": "tag-test-1",
        "release": "com.bullbitcoin.mobile@6.9.1+177",
        "platform": "other",
        "tags": {
            "category": "error",
            "migration_type": "install",
            "to_version": "6.9.1+177",
            "event.origin": "flutter"
        },
        "contexts": {
            "os": {"name": "Android", "version": "15"},
            "dev_message": "Failed to load wallet",
            "feature": {"flag": "wallet_v2", "enabled": true}
        }
    }"#.as_bytes().to_vec();
    let (hash, compressed) = compress_and_hash(&payload);
    let mut conn = pool.get().unwrap();
    ingest_use_case
        .execute(&mut conn, project_id, hash, compressed, None)
        .unwrap();

    assert_eq!(process_use_case.process_batch(10).unwrap(), 1);

    let tag_count: i64 = report_tag::table.count().get_result(&mut conn).unwrap();
    assert_eq!(tag_count, 4, "expected 4 tags persisted");

    // Only `dev_message` (primitive) + `feature` (object) — `os` is built-in
    // and goes to unwrap_os_*, not report_context.
    let context_count: i64 = report_context::table.count().get_result(&mut conn).unwrap();
    assert_eq!(context_count, 2, "expected 2 custom contexts persisted (os is built-in, skipped)");
}

#[test]
fn test_process_handles_missing_tags_and_contexts() {
    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );
    let process_use_case = DigestReportUseCase::new(repos, pool.clone(), compressor);

    // Event without tags or contexts — should still ingest cleanly
    let payload = r#"{"event_id": "no-tags-1", "platform": "other"}"#.as_bytes().to_vec();
    let (hash, compressed) = compress_and_hash(&payload);
    let mut conn = pool.get().unwrap();
    ingest_use_case
        .execute(&mut conn, project_id, hash, compressed, None)
        .unwrap();

    assert_eq!(process_use_case.process_batch(10).unwrap(), 1);
}

#[test]
fn test_process_coerces_non_string_tag_values() {
    use diesel::prelude::*;
    use crate::shared::persistence::db::schema::report_tag;

    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );
    let process_use_case = DigestReportUseCase::new(repos.clone(), pool.clone(), compressor);

    // Tag values are technically supposed to be strings; coerce numbers/bools,
    // skip nulls.
    let payload = r#"{
        "event_id": "coerce-1",
        "platform": "other",
        "tags": {
            "string_tag": "ok",
            "numeric_tag": 42,
            "bool_tag": true,
            "null_tag": null
        }
    }"#.as_bytes().to_vec();
    let (hash, compressed) = compress_and_hash(&payload);
    let mut conn = pool.get().unwrap();
    ingest_use_case
        .execute(&mut conn, project_id, hash, compressed, None)
        .unwrap();

    assert_eq!(process_use_case.process_batch(10).unwrap(), 1);

    // 3 stored: string + numeric + bool. Null skipped.
    let tag_count: i64 = report_tag::table.count().get_result(&mut conn).unwrap();
    assert_eq!(tag_count, 3, "expected 3 tags (null skipped)");
}

#[test]
fn test_process_extracts_breadcrumbs_with_dedup() {
    use diesel::prelude::*;
    use crate::shared::persistence::db::schema::{report_breadcrumb, unwrap_breadcrumb};

    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );
    let process_use_case = DigestReportUseCase::new(repos.clone(), pool.clone(), compressor);

    // Two events from the same session — bc[0] and bc[1] are physically the
    // same breadcrumbs (identical timestamp/category/type/level/data).
    // Event B has one extra trailing breadcrumb that A doesn't.
    let payload_a = r#"{
        "event_id": "ev-a",
        "platform": "other",
        "breadcrumbs": [
            {"timestamp":"2026-04-27T09:17:26.496Z","category":"app.lifecycle","type":"navigation","level":"info","data":{"state":"resumed"}},
            {"timestamp":"2026-04-27T09:17:26.800Z","category":"app.lifecycle","type":"navigation","level":"info","data":{"state":"paused"}}
        ]
    }"#.as_bytes().to_vec();
    let payload_b = r#"{
        "event_id": "ev-b",
        "platform": "other",
        "breadcrumbs": [
            {"timestamp":"2026-04-27T09:17:26.496Z","category":"app.lifecycle","type":"navigation","level":"info","data":{"state":"resumed"}},
            {"timestamp":"2026-04-27T09:17:26.800Z","category":"app.lifecycle","type":"navigation","level":"info","data":{"state":"paused"}},
            {"timestamp":"2026-04-27T09:17:27.050Z","category":"app.lifecycle","type":"navigation","level":"info","data":{"state":"resumed"}}
        ]
    }"#.as_bytes().to_vec();

    let (h_a, c_a) = compress_and_hash(&payload_a);
    let (h_b, c_b) = compress_and_hash(&payload_b);
    let mut conn = pool.get().unwrap();
    ingest_use_case.execute(&mut conn, project_id, h_a, c_a, None).unwrap();
    ingest_use_case.execute(&mut conn, project_id, h_b, c_b, None).unwrap();

    assert_eq!(process_use_case.process_batch(10).unwrap(), 2);

    // Join rows: 2 + 3 = 5
    let join_count: i64 = report_breadcrumb::table.count().get_result(&mut conn).unwrap();
    assert_eq!(join_count, 5, "expected 5 join rows (2 from A + 3 from B)");

    // Physical breadcrumbs: 3 (the two shared ones + one B-only).
    // Note: third B breadcrumb has identical category/type/level/data but a
    // different timestamp, so it's a distinct unwrap row by design.
    let unwrap_count: i64 = unwrap_breadcrumb::table.count().get_result(&mut conn).unwrap();
    assert_eq!(unwrap_count, 3, "expected 3 deduped unwrap rows");
}

#[test]
fn test_process_breadcrumbs_seq_preserved() {
    use diesel::prelude::*;
    use crate::shared::persistence::db::schema::{report_breadcrumb, unwrap_breadcrumb};

    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );
    let process_use_case = DigestReportUseCase::new(repos.clone(), pool.clone(), compressor);

    let payload = r#"{
        "event_id": "ev-seq",
        "platform": "other",
        "breadcrumbs": [
            {"timestamp":"2026-04-27T09:17:26.001Z","category":"a","type":"navigation","level":"info"},
            {"timestamp":"2026-04-27T09:17:26.002Z","category":"b","type":"navigation","level":"info"},
            {"timestamp":"2026-04-27T09:17:26.003Z","category":"c","type":"navigation","level":"info"}
        ]
    }"#.as_bytes().to_vec();
    let (h, c) = compress_and_hash(&payload);
    let mut conn = pool.get().unwrap();
    ingest_use_case.execute(&mut conn, project_id, h, c, None).unwrap();
    assert_eq!(process_use_case.process_batch(10).unwrap(), 1);

    let rows: Vec<(i32, i64)> = report_breadcrumb::table
        .inner_join(unwrap_breadcrumb::table)
        .select((report_breadcrumb::seq, unwrap_breadcrumb::timestamp.assume_not_null()))
        .order(report_breadcrumb::seq.asc())
        .load(&mut conn)
        .unwrap();
    assert_eq!(rows.len(), 3);
    // seq strictly increasing alongside timestamp
    assert_eq!(rows[0].0, 0);
    assert_eq!(rows[1].0, 1);
    assert_eq!(rows[2].0, 2);
    assert!(rows[0].1 < rows[1].1 && rows[1].1 < rows[2].1);
}

/// Exercise normalized JSONL imports with a deterministic, non-private fixture.
#[test]
fn e2e_crash_jsonl_breadcrumbs() {
    use diesel::prelude::*;
    use crate::shared::persistence::db::schema::{report_breadcrumb, unwrap_breadcrumb};

    let raw = concat!(
        "{\"event_id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"breadcrumbs\":[{\"timestamp\":\"2026-01-01T00:00:00Z\",\"message\":\"menu opened\"}]}\n",
        "{\"event_id\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"breadcrumbs\":[{\"timestamp\":\"2026-01-01T00:00:00Z\",\"message\":\"menu opened\"},{\"timestamp\":\"2026-01-01T00:00:01Z\",\"category\":\"navigation\",\"message\":\"course selected\"}]}\n",
        "{\"event_id\":\"cccccccccccccccccccccccccccccccc\",\"breadcrumbs\":[]}\n",
    );
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();

    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();
    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );
    let process_use_case = DigestReportUseCase::new(repos.clone(), pool.clone(), compressor);

    let mut conn = pool.get().unwrap();
    let mut expected_breadcrumb_total: usize = 0;
    let mut events_with_bc: usize = 0;
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).expect("bad jsonl line");
        if let Some(bc) = v.get("breadcrumbs").and_then(|b| b.as_array()) {
            expected_breadcrumb_total += bc.len();
            if !bc.is_empty() {
                events_with_bc += 1;
            }
        }
        let (h, c) = compress_and_hash(line.as_bytes());
        ingest_use_case
            .execute(&mut conn, project_id, h, c, None)
            .unwrap();
    }
    process_use_case.process_batch(1000).unwrap();

    let join_count: i64 = report_breadcrumb::table.count().get_result(&mut conn).unwrap();
    let unwrap_count: i64 = unwrap_breadcrumb::table.count().get_result(&mut conn).unwrap();

    println!(
        "crash.jsonl: {} lines, {} events with breadcrumbs, {} breadcrumbs in source",
        lines.len(),
        events_with_bc,
        expected_breadcrumb_total
    );
    println!(
        "DB: {} report_breadcrumb rows, {} unwrap_breadcrumb rows (dedup ratio = {:.2}x)",
        join_count,
        unwrap_count,
        join_count as f64 / unwrap_count.max(1) as f64
    );

    assert_eq!(
        join_count as usize, expected_breadcrumb_total,
        "join rows must equal total source breadcrumbs"
    );
    assert!(
        unwrap_count <= join_count,
        "unwrap_count cannot exceed join_count"
    );
}

#[test]
fn test_process_handles_missing_breadcrumbs() {
    use diesel::prelude::*;
    use crate::shared::persistence::db::schema::report_breadcrumb;

    let (repos, pool, project_id) = setup_test_db();
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );
    let process_use_case = DigestReportUseCase::new(repos.clone(), pool.clone(), compressor);

    let payload = r#"{"event_id":"ev-no-bc","platform":"other"}"#.as_bytes().to_vec();
    let (h, c) = compress_and_hash(&payload);
    let mut conn = pool.get().unwrap();
    ingest_use_case.execute(&mut conn, project_id, h, c, None).unwrap();
    assert_eq!(process_use_case.process_batch(10).unwrap(), 1);

    let join_count: i64 = report_breadcrumb::table.count().get_result(&mut conn).unwrap();
    assert_eq!(join_count, 0);
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

    let process_use_case = DigestReportUseCase::new(repos, pool.clone(), compressor);

    let payload1 = r#"{"event_id": "e1", "release": "app@1.0.0", "platform": "python"}"#.as_bytes();
    let payload2 = r#"{"event_id": "e2", "release": "app@2.0.0", "platform": "rust"}"#.as_bytes();
    let payload3 = r#"{"event_id": "e3", "release": "app@3.0.0", "platform": "go"}"#.as_bytes();

    let (h1, c1) = compress_and_hash(payload1);
    let (h2, c2) = compress_and_hash(payload2);
    let (h3, c3) = compress_and_hash(payload3);

    let mut conn = pool.get().unwrap();
    ingest_use_case
        .execute(&mut conn, project_id, h1, c1, None)
        .unwrap();
    ingest_use_case
        .execute(&mut conn, project_id, h2, c2, None)
        .unwrap();
    ingest_use_case
        .execute(&mut conn, project_id, h3, c3, None)
        .unwrap();

    assert_eq!(queue_repo.count_pending(&mut conn).unwrap(), 3);

    let processed = process_use_case.process_batch(10).unwrap();
    assert_eq!(processed, 3);

    assert_eq!(queue_repo.count_pending(&mut conn).unwrap(), 0);
}

#[test]
fn protocol_event_retries_are_idempotent_and_project_scoped() {
    let (repos, pool, first_project) = setup_test_db();
    let second_project = repos.project.create(None, None).unwrap();
    let ingest = IngestReportUseCase::new(repos.archive.clone(), repos.queue.clone(), repos.project.clone());
    let digest = DigestReportUseCase::new(repos.clone(), pool.clone(), GzipCompressor::new());
    let mut conn = pool.get().unwrap();
    for (project, attempt, event_id) in [
        (first_project, 1, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        (first_project, 2, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        (second_project, 3, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        (first_project, 4, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    ] {
        let payload = serde_json::to_vec(&serde_json::json!({
            "event_id": event_id, "timestamp": "2026-01-22T10:00:00Z",
            "extra": {"attempt": attempt}, "message": "same error"
        })).unwrap();
        let (hash, compressed) = compress_and_hash(&payload);
        ingest.execute(&mut conn, project, hash, compressed, None).unwrap();
        digest.process_batch(10).unwrap();
    }
    assert_eq!(repos.report.count_by_project(first_project).unwrap(), 2);
    assert_eq!(repos.report.count_by_project(second_project).unwrap(), 1);
    assert_eq!(repos.queue_error.count(&mut conn).unwrap(), 0);
}

#[test]
fn protocol_envelope_header_event_id_is_preserved() {
    let (repos, pool, project_id) = setup_test_db();
    let ingest = IngestReportUseCase::new(repos.archive.clone(), repos.queue.clone(), repos.project.clone());
    let digest = DigestReportUseCase::new(repos.clone(), pool.clone(), GzipCompressor::new());
    let payload = b"{\"event_id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}\n{\"type\":\"event\"}\n{\"message\":\"test\"}";
    let (hash, compressed) = compress_and_hash(payload);
    ingest.execute(&mut pool.get().unwrap(), project_id, hash, compressed, None).unwrap();
    digest.process_batch(10).unwrap();
    assert!(repos.report.find_by_event_id("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap().is_some());
}

#[test]
fn protocol_explicit_fingerprint_groups_without_collapsing_occurrences() {
    use diesel::prelude::*;
    use crate::shared::persistence::db::schema::report;
    let (repos, pool, project_id) = setup_test_db();
    let ingest = IngestReportUseCase::new(repos.archive.clone(), repos.queue.clone(), repos.project.clone());
    let digest = DigestReportUseCase::new(repos.clone(), pool.clone(), GzipCompressor::new());
    let mut conn = pool.get().unwrap();
    for (event_id, fingerprint) in [("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "network-timeout"), ("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "network-timeout"), ("cccccccccccccccccccccccccccccccc", "storage-failed")] {
        let payload = serde_json::to_vec(&serde_json::json!({"event_id":event_id,"fingerprint":[fingerprint],"message":"operation failed"})).unwrap();
        let (hash, compressed) = compress_and_hash(&payload);
        ingest.execute(&mut conn, project_id, hash, compressed, None).unwrap();
        digest.process_batch(10).unwrap();
    }
    let issues: Vec<Option<i32>> = report::table.order(report::event_id).select(report::issue_id).load(&mut conn).unwrap();
    assert_eq!(issues.len(), 3);
    assert!(issues[0].is_some());
    assert_eq!(issues[0], issues[1]);
    assert_ne!(issues[1], issues[2]);
}

#[test]
fn protocol_usage_event_does_not_create_an_error_issue() {
    let (repos, pool, project_id) = setup_test_db();
    let ingest = IngestReportUseCase::new(repos.archive.clone(), repos.queue.clone(), repos.project.clone());
    let digest = DigestReportUseCase::new(repos.clone(), pool.clone(), GzipCompressor::new());
    let payload = br#"{"event_id":"dddddddddddddddddddddddddddddddd","level":"info","message":"app.session.started","tags":{"event_kind":"app_session"},"user":{"id":"test-installation"}}"#;
    let (hash, compressed) = compress_and_hash(payload);
    ingest.execute(&mut pool.get().unwrap(), project_id, hash, compressed, None).unwrap();
    digest.process_batch(10).unwrap();
    let report = repos.report.find_by_event_id("dddddddddddddddddddddddddddddddd").unwrap().unwrap();
    assert!(report.issue_id.is_none());
    assert!(report.user_id.is_some());
}
