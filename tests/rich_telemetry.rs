use axum::{body::Body, http::{Request, StatusCode}, Router};
use crash_cache::features::{digest::DigestReportUseCase, ingest::{AppState, HealthStats, ProjectCache, IngestReportUseCase, create_api_router}};
use crash_cache::shared::{compression::GzipCompressor, persistence::{DbPool, Repositories, establish_connection_pool, run_migrations}};
use diesel::{prelude::*, sql_types::{BigInt, Integer}};
use serde_json::{Value, json};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Semaphore;
use tower::ServiceExt;

fn setup() -> (Router, DbPool, Repositories, i32) {
    let url = std::env::var("DATABASE_URL").expect("isolated DATABASE_URL required");
    assert!(url.contains("crash_cache_test"));
    let pool = establish_connection_pool(&url, 4, 10);
    run_migrations(&pool);
    let repos = Repositories::new(pool.clone());
    let project = repos.project.create(None, None).unwrap();
    let state = AppState {
        ingest_use_case: IngestReportUseCase::new(repos.archive.clone(), repos.queue.clone(), repos.project.clone()),
        compression_semaphore: Arc::new(Semaphore::new(1)), pool: pool.clone(),
        project_repo: repos.project.clone(), project_cache: ProjectCache::new(Duration::from_secs(1)),
        health_cache: Arc::new(RwLock::new(HealthStats::default())), health_cache_ttl: Duration::from_secs(1),
        max_uncompressed_payload_bytes: 4 * 1024 * 1024, session_repo: repos.session.clone(),
        session_status_repo: repos.session_status.clone(), session_release_repo: repos.session_release.clone(),
        session_environment_repo: repos.session_environment.clone(),
    };
    (create_api_router(state), pool, repos, project)
}

fn envelope(header: Value, items: Vec<(Value, Vec<u8>)>) -> Vec<u8> {
    let mut bytes = format!("{header}\n").into_bytes();
    for (mut header, payload) in items {
        header["length"] = json!(payload.len());
        bytes.extend_from_slice(format!("{header}\n").as_bytes());
        bytes.extend_from_slice(&payload);
        bytes.push(b'\n');
    }
    bytes
}
fn log() -> Value {
    json!({"timestamp":1790424000.125,"trace_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","span_id":"bbbbbbbbbbbbbbbb","level":"info","body":"map loaded","severity_number":9,"attributes":{"layer":{"type":"string","value":"flutter"},"piece_count":{"type":"integer","value":12},"modes":{"type":"array","value":["creative","test"]}}})
}
fn logs(values: Vec<Value>) -> (Value,Vec<u8>) {
    (json!({"type":"log","item_count":values.len(),"content_type":"application/vnd.sentry.items.log+json"}), serde_json::to_vec(&json!({"items":values})).unwrap())
}
async fn send(router: &Router, project: i32, body: Vec<u8>) -> StatusCode {
    router.clone().oneshot(Request::builder().method("POST")
        .uri(format!("/api/{project}/envelope/?sentry_key=test"))
        .body(Body::from(body)).unwrap()).await.unwrap().status()
}
fn count(pool: &DbPool, project: i32, table: &str) -> i64 {
    #[derive(QueryableByName)] struct Count { #[diesel(sql_type = BigInt)] value: i64 }
    diesel::sql_query(format!("SELECT count(*) AS value FROM {table} WHERE project_id=$1"))
        .bind::<Integer,_>(project).get_result::<Count>(&mut pool.get().unwrap()).unwrap().value
}
fn digest(pool: &DbPool, repos: &Repositories) {
    DigestReportUseCase::new(repos.clone(), pool.clone(), GzipCompressor::new()).process_batch(1000).unwrap();
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn rich_logs_are_not_errors_and_exact_retries_are_idempotent() {
    let (router,pool,repos,project) = setup();
    let batch = envelope(json!({}),vec![logs(vec![log(),log()])]);
    for _ in 0..2 { assert_eq!(send(&router,project,batch.clone()).await,StatusCode::OK); digest(&pool,&repos); }
    assert_eq!(count(&pool,project,"telemetry_log"),2,"identical entries in one batch are distinct occurrences");
    assert_eq!(count(&pool,project,"report"),0,"logs must not manufacture error events");
    let other_project = repos.project.create(None,None).unwrap();
    assert_eq!(send(&router,other_project,batch).await,StatusCode::OK); digest(&pool,&repos);
    assert_eq!(count(&pool,other_project,"telemetry_log"),2);
    let another_batch = envelope(json!({"sent_at":"2026-09-26T12:01:00Z"}),vec![logs(vec![log()])]);
    assert_eq!(send(&router,project,another_batch).await,StatusCode::OK); digest(&pool,&repos);
    assert_eq!(count(&pool,project,"telemetry_log"),3,"a new envelope must not heuristically discard identical log content");
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn rich_mixed_sessions_logs_and_invalid_batches_are_atomic() {
    let (router,pool,repos,project) = setup();
    let sid = uuid::Uuid::new_v4().to_string();
    let session = json!({"sid":sid,"did":"synthetic-installation","init":true,"seq":1,"started":"2026-09-26T12:00:00Z","status":"ok","errors":0,"attrs":{"release":"test@1"}});
    let session_item = || (json!({"type":"session"}),serde_json::to_vec(&session).unwrap());
    let bad = envelope(json!({}),vec![session_item(),logs(vec![log(),json!({"body":"missing required fields"})])]);
    assert_eq!(send(&router,project,bad).await,StatusCode::BAD_REQUEST);
    assert_eq!(count(&pool,project,"session"),0);
    assert_eq!(count(&pool,project,"archive"),0,"invalid batch must not be partially admitted");
    let good = envelope(json!({}),vec![session_item(),logs(vec![log()])]);
    assert_eq!(send(&router,project,good).await,StatusCode::OK); digest(&pool,&repos);
    assert_eq!(count(&pool,project,"session"),1);
    assert_eq!(count(&pool,project,"telemetry_log"),1);
    assert_eq!(count(&pool,project,"report"),0);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn rich_attachments_and_frame_context_survive_event_replay() {
    let (router,pool,repos,project) = setup();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let event = json!({"event_id":id,"platform":"other","exception":{"values":[{"type":"SyntheticFailure","value":"probe","stacktrace":{"frames":[{"filename":"probe.gd","lineno":4,"vars":{"piece_count":12},"context_line":"fail()","pre_context":["start()"],"post_context":["stop()"]}]}}]}});
    let event_item = || (json!({"type":"event"}),serde_json::to_vec(&event).unwrap());
    let attachment = || (json!({"type":"attachment","filename":"screenshot.png","content_type":"image/png","attachment_type":"event.attachment"}),b"\x89PNG\x00\nopaque".to_vec());
    let first = envelope(json!({"event_id":id}),vec![event_item(),attachment()]);
    for _ in 0..2 { assert_eq!(send(&router,project,first.clone()).await,StatusCode::OK); digest(&pool,&repos); }
    assert_eq!(count(&pool,project,"attachment_metadata"),1);
    let next = envelope(json!({"event_id":id}),vec![event_item(),logs(vec![log()]),(json!({"type":"attachment","filename":"scene.json","content_type":"application/json"}),b"{\"nodes\":[]}".to_vec())]);
    assert_eq!(send(&router,project,next).await,StatusCode::OK); digest(&pool,&repos);
    assert_eq!(count(&pool,project,"report"),1);
    assert_eq!(count(&pool,project,"attachment_metadata"),2,"duplicate event cannot discard new attachment metadata");
    assert_eq!(count(&pool,project,"telemetry_log"),1);
    let late = envelope(json!({"event_id":id}),vec![attachment()]);
    assert_eq!(send(&router,project,late).await,StatusCode::OK); digest(&pool,&repos);
    assert_eq!(count(&pool,project,"attachment_metadata"),3);
    assert_eq!(count(&pool,project,"report"),1);
    #[derive(QueryableByName)] struct Frames { #[diesel(sql_type = diesel::sql_types::Jsonb)] frames: Value }
    let saved = diesel::sql_query("SELECT s.frames FROM unwrap_stacktrace s JOIN report r ON r.stacktrace_id=s.id WHERE r.project_id=$1")
        .bind::<Integer,_>(project).get_result::<Frames>(&mut pool.get().unwrap()).unwrap();
    assert_eq!(saved.frames[0]["vars"]["piece_count"],12);
    assert_eq!(saved.frames[0]["context_line"],"fail()");
    #[derive(QueryableByName)] struct Metadata { #[diesel(sql_type = BigInt)] bytes: i64 }
    let saved = diesel::sql_query("SELECT size_bytes AS bytes FROM attachment_metadata WHERE project_id=$1 AND filename='screenshot.png' ORDER BY id LIMIT 1")
        .bind::<Integer,_>(project).get_result::<Metadata>(&mut pool.get().unwrap()).unwrap();
    assert_eq!(saved.bytes,b"\x89PNG\x00\nopaque".len() as i64);
    let archived = repos.archive.find_by_hash(&mut pool.get().unwrap(), &{
        #[derive(QueryableByName)] struct Hash { #[diesel(sql_type = diesel::sql_types::Text)] hash: String }
        diesel::sql_query("SELECT archive_hash AS hash FROM attachment_metadata WHERE project_id=$1 ORDER BY id LIMIT 1")
            .bind::<Integer,_>(project).get_result::<Hash>(&mut pool.get().unwrap()).unwrap().hash
    }).unwrap().unwrap();
    let bytes = GzipCompressor::new().decompress(&archived.compressed_payload).unwrap();
    assert_eq!(bytes,first,"binary payload remains byte-exact in its archived envelope");
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn rich_ingestion_preserves_raw_store_events() {
    let (router,pool,repos,project) = setup();
    let event = json!({"event_id":uuid::Uuid::new_v4().simple().to_string(),"platform":"other","message":"raw-store regression"});
    let response = router.oneshot(Request::builder().method("POST")
        .uri(format!("/api/{project}/store/?sentry_key=test"))
        .body(Body::from(serde_json::to_vec(&event).unwrap())).unwrap()).await.unwrap();
    assert_eq!(response.status(),StatusCode::OK); digest(&pool,&repos);
    assert_eq!(count(&pool,project,"report"),1);
    assert_eq!(count(&pool,project,"telemetry_log"),0);
}

#[test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL with CREATE SCHEMA"]
fn rich_migration_up_down_up_preserves_published_tables() {
    use diesel::connection::SimpleConnection;
    let url = std::env::var("DATABASE_URL").expect("isolated DATABASE_URL required");
    assert!(url.contains("crash_cache_test"));
    let pool = establish_connection_pool(&url,1,10);
    pool.get().unwrap().test_transaction::<_,diesel::result::Error,_>(|conn| {
        conn.batch_execute("CREATE SCHEMA rich_migration_regression; SET LOCAL search_path TO rich_migration_regression;")?;
        conn.batch_execute(include_str!("../migrations/00000000000000_initial_schema/up.sql"))?;
        conn.batch_execute(include_str!("../migrations/20260926000000_sentry_identity_sessions/up.sql"))?;
        conn.batch_execute("INSERT INTO project(id,created_at) VALUES (1,CURRENT_TIMESTAMP)")?;
        conn.batch_execute(include_str!("../migrations/20260926000001_rich_telemetry/up.sql"))?;
        conn.batch_execute(include_str!("../migrations/20260926000001_rich_telemetry/down.sql"))?;
        conn.batch_execute(include_str!("../migrations/20260926000001_rich_telemetry/up.sql"))?;
        #[derive(QueryableByName)] struct Count { #[diesel(sql_type = BigInt)] value: i64 }
        let saved = diesel::sql_query("SELECT count(*) AS value FROM project WHERE id=1").get_result::<Count>(conn)?;
        assert_eq!(saved.value,1);
        Ok(())
    });
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn rich_thread_frames_preserve_variables_without_guessing_the_thread() {
    let (router,pool,repos,project) = setup();
    let thread_stack = json!({"frames":[{"filename":"probe.gd","function":"fail","lineno":12,"vars":{"piece_count":12}}]});
    let cases = [
        (json!([{"id":7,"stacktrace":thread_stack.clone()}]),None,true),
        (json!([{"id":8,"stacktrace":thread_stack.clone()}]),None,false),
        (json!([{"id":7,"stacktrace":thread_stack.clone()},{"id":"7","stacktrace":thread_stack.clone()}]),None,false),
        (json!([{"id":7,"stacktrace":thread_stack.clone()}]),Some(json!({"frames":[{"filename":"explicit.gd","vars":{"piece_count":99}}]})),true),
    ];
    for (threads,explicit,has_stack) in cases {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let mut exception = json!({"type":"ThreadFailure","value":"synthetic thread fixture","thread_id":7});
        if let Some(stack) = &explicit { exception["stacktrace"] = stack.clone(); }
        let event = json!({"event_id":id,"platform":"other","exception":{"values":[exception]},"threads":{"values":threads}});
        let payload = envelope(json!({"event_id":id}),vec![(json!({"type":"event"}),serde_json::to_vec(&event).unwrap())]);
        assert_eq!(send(&router,project,payload).await,StatusCode::OK);
        digest(&pool,&repos);
        #[derive(QueryableByName)] struct Saved {
            #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Jsonb>)] frames: Option<Value>
        }
        let saved = diesel::sql_query("SELECT s.frames FROM report r LEFT JOIN unwrap_stacktrace s ON r.stacktrace_id=s.id WHERE r.project_id=$1 AND r.event_id=$2")
            .bind::<Integer,_>(project).bind::<diesel::sql_types::Text,_>(&id)
            .get_result::<Saved>(&mut pool.get().unwrap()).unwrap();
        assert_eq!(saved.frames.is_some(),has_stack,"only explicit stacks or unique thread references may supply frames");
        if let Some(frames) = saved.frames {
            assert_eq!(frames[0]["vars"]["piece_count"],if explicit.is_some() { 99 } else { 12 });
            assert!(frames[0].get("context_line").is_none(),"source context absent from the SDK must not be invented");
        }
    }
}
