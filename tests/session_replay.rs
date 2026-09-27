use crash_cache::features::digest::DigestReportUseCase;
use crash_cache::features::ingest::IngestReportUseCase;
use crash_cache::shared::compression::GzipCompressor;
use crash_cache::shared::persistence::{establish_connection_pool, run_migrations, Repositories};
use crash_cache::shared::persistence::db::schema::{issue, report};
use diesel::prelude::*;
use sha2::{Digest, Sha256};

#[test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
fn duplicate_event_does_not_discard_a_newer_bundled_session() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = establish_connection_pool(&url, 4, 10);
    run_migrations(&pool);
    let repos = Repositories::new(pool.clone());
    let project_id = repos.project.create(None, None).unwrap();
    let ingest = IngestReportUseCase::new(repos.archive.clone(), repos.queue.clone(), repos.project.clone());
    let digest = DigestReportUseCase::new(repos.clone(), pool.clone(), GzipCompressor::new());
    let sid = uuid::Uuid::new_v4().to_string();
    let event_id = uuid::Uuid::new_v4().simple().to_string();
    let mut conn = pool.get().unwrap();
    for (seq, status, errors) in [(1, "ok", 1), (2, "crashed", 2)] {
        let event = serde_json::json!({
            "event_id": event_id,
            "platform": "rust",
            "exception": {"values": [{"type": "ReplayRegression", "value": "shared failure"}]}
        });
        let session = serde_json::json!({
            "sid": sid, "did": "installation-id", "seq": seq,
            "started": "2026-09-26T10:00:00Z", "timestamp": "2026-09-26T10:01:00Z",
            "status": status, "errors": errors, "attrs": {"release": "game@1"}
        });
        let envelope = format!("{{}}\n{{\"type\":\"event\"}}\n{event}\n{{\"type\":\"session\"}}\n{session}\n");
        let compressed = GzipCompressor::new().compress(envelope.as_bytes()).unwrap();
        let hash = hex::encode(Sha256::digest(&compressed));
        ingest.execute(&mut conn, project_id, hash, compressed, None).unwrap();
        digest.process_batch(100).unwrap();
    }
    let stored = repos.session.find_by_sid_with_conn(&mut conn, project_id, &sid).unwrap().unwrap();
    let status = repos.session_status.find_by_id(&mut conn, stored.status_id).unwrap().unwrap();
    assert_eq!(status.value, "crashed", "event deduplication must not roll back a session update");
    assert_eq!(stored.sequence, "2");
    assert_eq!(stored.errors, 2);
    let (count, issue_id): (i64, Option<i32>) = report::table
        .filter(report::project_id.eq(project_id))
        .select((diesel::dsl::count_star(), diesel::dsl::max(report::issue_id)))
        .first(&mut conn).unwrap();
    assert_eq!(count, 1);
    let occurrences: i32 = issue::table.find(issue_id.unwrap()).select(issue::event_count).first(&mut conn).unwrap();
    assert_eq!(occurrences, 1, "a duplicate event must not increment issue occurrences");
}
