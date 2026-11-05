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
        domain::QueueItem,
        persistence::{DbPool, Repositories, establish_connection_pool, run_migrations},
    },
};
use diesel::prelude::*;
use serde_json::{Value, json};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use tower::ServiceExt;

struct Fixture {
    pool: DbPool,
    repos: Repositories,
    project: i32,
}

impl Fixture {
    fn new() -> Self {
        let url = std::env::var("DATABASE_URL").expect("isolated DATABASE_URL required");
        assert!(url.contains("crash_cache_test"));
        let pool = establish_connection_pool(&url, 4, 5);
        run_migrations(&pool);
        let repos = Repositories::new(pool.clone());
        let project = repos.project.create(None, None).unwrap();
        Self {
            pool,
            repos,
            project,
        }
    }

    async fn send(&self, sessions: &[Value], with_event: bool) -> StatusCode {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let mut bytes = format!("{}\n", json!({"event_id":id})).into_bytes();
        if with_event {
            append_item(
                &mut bytes,
                "event",
                &json!({"event_id":id,"message":"valid event"}),
            );
        }
        for session in sessions {
            append_item(&mut bytes, "session", session);
        }
        let state = AppState {
            ingest_use_case: IngestReportUseCase::new(
                self.repos.archive.clone(),
                self.repos.queue.clone(),
                self.repos.project.clone(),
            ),
            compression_semaphore: Arc::new(Semaphore::new(1)),
            pool: self.pool.clone(),
            project_repo: self.repos.project.clone(),
            project_cache: ProjectCache::new(Duration::from_secs(1)),
            health_cache: Arc::new(RwLock::new(HealthStats::default())),
            health_cache_ttl: Duration::from_secs(1),
            max_uncompressed_payload_bytes: 4096,
            session_repo: self.repos.session.clone(),
            session_status_repo: self.repos.session_status.clone(),
            session_release_repo: self.repos.session_release.clone(),
            session_environment_repo: self.repos.session_environment.clone(),
        };
        let response = create_api_router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/{}/envelope/?sentry_key=test", self.project))
                    .body(Body::from(bytes.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let archives = self.archives();
        assert!(
            archives.iter().any(|(_, payload)| {
                GzipCompressor::new().decompress(payload).unwrap() == bytes
            }),
            "the complete envelope must survive projection failures"
        );
        response.status()
    }

    fn archives(&self) -> Vec<(String, Vec<u8>)> {
        use crash_cache::shared::persistence::db::schema::archive;
        archive::table
            .filter(archive::project_id.eq(self.project))
            .select((archive::hash, archive::compressed_payload))
            .load(&mut self.pool.get().unwrap())
            .unwrap()
    }

    fn digest(&self) -> u32 {
        DigestReportUseCase::new(self.repos.clone(), self.pool.clone(), GzipCompressor::new())
            .process_batch(100)
            .unwrap()
    }

    fn reports(&self) -> i64 {
        self.repos.report.count_by_project(self.project).unwrap()
    }
    fn sessions(&self) -> i64 {
        self.repos.session.count_by_project(self.project).unwrap()
    }

    fn replay(&self) {
        let hash = self.archives()[0].0.clone();
        self.repos
            .queue
            .enqueue(&mut self.pool.get().unwrap(), &QueueItem::new(hash))
            .unwrap();
    }
}

fn append_item(bytes: &mut Vec<u8>, kind: &str, value: &Value) {
    let payload = serde_json::to_vec(value).unwrap();
    bytes
        .extend_from_slice(format!("{}\n", json!({"type":kind,"length":payload.len()})).as_bytes());
    bytes.extend_from_slice(&payload);
    bytes.push(b'\n');
}

fn valid_session() -> Value {
    json!({"sid":uuid::Uuid::new_v4().to_string(),"started":"2026-11-04T10:00:00Z",
        "seq":1,"status":"crashed","errors":1,"attrs":{"release":"test@1"}})
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn acknowledged_valid_event_is_not_lost_due_to_an_invalid_session() {
    let fixture = Fixture::new();
    let mut session = valid_session();
    session["sid"] = json!("not-a-uuid");
    fixture.send(&[session], true).await;
    let processed = fixture.digest();
    assert_eq!(
        fixture.reports(),
        1,
        "acknowledged event was lost; digest processed={processed}"
    );
    assert_eq!(fixture.sessions(), 0);
    fixture.replay();
    fixture.digest();
    assert_eq!(
        fixture.reports(),
        1,
        "replaying must not duplicate the event"
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn acknowledged_valid_event_survives_exited_session_with_optional_sid() {
    let fixture = Fixture::new();
    let session =
        json!({"status":"exited","started":"2026-11-04T10:00:00Z","attrs":{"release":"test@1"}});
    fixture.send(&[session], true).await;
    let processed = fixture.digest();
    assert_eq!(
        fixture.reports(),
        1,
        "acknowledged event was lost; digest processed={processed}"
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn invalid_session_does_not_discard_valid_siblings() {
    let fixture = Fixture::new();
    let mut invalid = valid_session();
    invalid["started"] = json!("not-a-date");
    fixture.send(&[invalid, valid_session()], true).await;
    fixture.digest();
    assert_eq!(fixture.reports(), 1);
    assert_eq!(fixture.sessions(), 1);
    fixture.replay();
    fixture.digest();
    assert_eq!(fixture.reports(), 1);
    assert_eq!(fixture.sessions(), 1);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn session_sql_failure_does_not_abort_event_or_other_sessions() {
    let fixture = Fixture::new();
    let rejected = valid_session();
    let sid = rejected["sid"].as_str().unwrap();
    // A per-project constraint injects a real SQL error without affecting other fixtures.
    let constraint = format!("session_review_{}", fixture.project);
    diesel::sql_query(format!("ALTER TABLE session ADD CONSTRAINT {constraint} CHECK (project_id <> {} OR sid <> '{sid}')", fixture.project))
        .execute(&mut fixture.pool.get().unwrap()).unwrap();
    fixture.send(&[rejected, valid_session()], true).await;
    let processed = fixture.digest();
    diesel::sql_query(format!("ALTER TABLE session DROP CONSTRAINT {constraint}"))
        .execute(&mut fixture.pool.get().unwrap())
        .unwrap();
    assert_eq!(processed, 1);
    assert_eq!(fixture.reports(), 1);
    assert_eq!(fixture.sessions(), 1);
    // After repairing the projection, the same archive recovers the failed session.
    fixture.replay();
    fixture.digest();
    assert_eq!(fixture.reports(), 1);
    assert_eq!(fixture.sessions(), 2);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn session_only_statistics_can_be_rebuilt_from_archives() {
    let fixture = Fixture::new();
    fixture.send(&[valid_session()], false).await;
    assert_eq!(
        fixture.sessions(),
        0,
        "ingestion must archive before projecting"
    );
    assert_eq!(fixture.digest(), 1);
    assert_eq!(fixture.sessions(), 1);
    assert_eq!(fixture.reports(), 0);
    use crash_cache::shared::persistence::db::schema::session;
    diesel::delete(session::table.filter(session::project_id.eq(fixture.project)))
        .execute(&mut fixture.pool.get().unwrap())
        .unwrap();
    fixture.replay();
    assert_eq!(fixture.digest(), 1);
    assert_eq!(
        fixture.sessions(),
        1,
        "the archive must rebuild the session projection"
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn session_only_envelope_retains_uninterpretable_items_and_projects_valid_ones() {
    let fixture = Fixture::new();
    fixture
        .send(&[json!({"unrecognized":"session"}), valid_session()], false)
        .await;
    assert_eq!(fixture.digest(), 1);
    assert_eq!(fixture.sessions(), 1);
    assert_eq!(fixture.reports(), 0);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn archived_sessions_keep_the_highest_sequence_in_any_replay_order() {
    let fixture = Fixture::new();
    let mut newest = valid_session();
    newest["seq"] = json!(10);
    let mut oldest = newest.clone();
    oldest["seq"] = json!(1);
    oldest["status"] = json!("ok");
    oldest["errors"] = json!(0);
    fixture.send(&[newest.clone()], false).await;
    fixture.digest();
    fixture.send(&[oldest], false).await;
    fixture.digest();
    let check = || {
        let row = fixture
            .repos
            .session
            .find_by_sid(fixture.project, newest["sid"].as_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(row.sequence, "10");
        assert_eq!(row.errors, 1);
    };
    check();
    assert_eq!(fixture.archives().len(), 2);
    for (hash, _) in fixture.archives().into_iter().rev() {
        fixture
            .repos
            .queue
            .enqueue(&mut fixture.pool.get().unwrap(), &QueueItem::new(hash))
            .unwrap();
        fixture.digest();
    }
    check();
    assert_eq!(fixture.sessions(), 1);
    assert_eq!(fixture.reports(), 0);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn wholly_uninterpretable_session_envelope_is_still_archived() {
    let fixture = Fixture::new();
    fixture
        .send(&[json!({"future":"session format"})], false)
        .await;
    assert_eq!(fixture.archives().len(), 1);
    assert_eq!(fixture.digest(), 1);
    assert_eq!(fixture.sessions(), 0);
    assert_eq!(fixture.reports(), 0);
}
