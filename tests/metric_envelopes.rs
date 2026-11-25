use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use crash_cache::{
    features::ingest::{
        AppState, HealthStats, IngestReportUseCase, ProjectCache, create_api_router,
    },
    shared::persistence::{DbPool, Repositories, establish_connection_pool, run_migrations},
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Double},
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use tower::ServiceExt;

struct Fixture {
    pool: DbPool,
    state: AppState,
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
        let state = AppState {
            ingest_use_case: IngestReportUseCase::new(
                repos.archive.clone(),
                repos.queue.clone(),
                repos.project.clone(),
            ),
            compression_semaphore: Arc::new(Semaphore::new(4)),
            pool: pool.clone(),
            project_repo: repos.project.clone(),
            project_cache: ProjectCache::new(Duration::from_secs(1)),
            health_cache: Arc::new(RwLock::new(HealthStats::default())),
            health_cache_ttl: Duration::from_secs(1),
            max_uncompressed_payload_bytes: 65536,
            session_repo: repos.session.clone(),
            session_status_repo: repos.session_status.clone(),
            session_release_repo: repos.session_release.clone(),
            session_environment_repo: repos.session_environment.clone(),
        };
        Self {
            pool,
            state,
            project,
        }
    }
    async fn send(&self, body: String) -> StatusCode {
        create_api_router(self.state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/{}/envelope/?sentry_key=test", self.project))
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }
    fn counts(&self) -> Counts {
        diesel::sql_query("SELECT (SELECT count(*) FROM archive WHERE project_id=$1) AS archives, (SELECT count(*) FROM metric_replay WHERE project_id=$1) AS replays, (SELECT COALESCE(sum(sample_count),0)::bigint FROM metric_hour WHERE project_id=$1) AS samples, (SELECT COALESCE(sum(value_sum),0)::double precision FROM metric_hour WHERE project_id=$1) AS total")
            .bind::<diesel::sql_types::Integer,_>(self.project).get_result(&mut self.pool.get().unwrap()).unwrap()
    }
}
#[derive(QueryableByName, Debug)]
struct Counts {
    #[diesel(sql_type=BigInt)]
    archives: i64,
    #[diesel(sql_type=BigInt)]
    replays: i64,
    #[diesel(sql_type=BigInt)]
    samples: i64,
    #[diesel(sql_type=Double)]
    total: f64,
}
fn point() -> Value {
    json!({
        "timestamp": chrono::Utc::now().timestamp(), "trace_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "name":"bull.installation.active", "type":"counter", "value":1,
        "attributes": {"sentry.release":{"type":"string","value":"6.16.0+220"}, "sentry.environment":{"type":"string","value":"regtest"}}
    })
}
fn envelope(points: Vec<Value>) -> String {
    format!(
        "{{}}\n{}\n{}",
        json!({"type":"trace_metric","item_count":points.len(),"content_type":"application/vnd.sentry.items.trace-metric+json"}),
        json!({"items":points})
    )
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn accepts_a_valid_metric_envelope() {
    let f = Fixture::new();
    assert_eq!(f.send(envelope(vec![point()])).await, StatusCode::OK);
    let c = f.counts();
    assert_eq!((c.archives, c.replays, c.samples, c.total), (0, 1, 1, 1.0));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn replay_and_aggregates_are_atomic() {
    let f = Arc::new(Fixture::new());
    let batch = envelope(vec![point(), point(), point()]);
    let first = {
        let f = f.clone();
        let batch = batch.clone();
        tokio::spawn(async move { f.send(batch).await })
    };
    let second = {
        let f = f.clone();
        let batch = batch.clone();
        tokio::spawn(async move { f.send(batch).await })
    };
    let (a, b) = tokio::join!(first, second);
    assert_eq!((a.unwrap(), b.unwrap()), (StatusCode::OK, StatusCode::OK));
    assert_eq!(f.send(batch).await, StatusCode::OK);
    let c = f.counts();
    assert_eq!((c.archives, c.replays, c.samples, c.total), (0, 1, 3, 3.0));
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn invalid_batches_and_mixed_envelopes_are_never_stored() {
    let f = Fixture::new();
    let valid = point();
    let mut cases = Vec::new();
    let mut p = point();
    p["attributes"]["user.id"] = json!({"type":"string","value":"secret"});
    cases.push(p);
    let mut p = point();
    p["attributes"]["from_schema"] = json!({"type":"integer","value":1025});
    cases.push(p);
    let mut p = point();
    p["attributes"]["from_schema"] = json!({"type":"string","value":"1"});
    cases.push(p);
    let mut p = point();
    p["attributes"]["sentry.environment"]["value"] = json!("other");
    cases.push(p);
    let mut p = point();
    p["attributes"]["sentry.release"]["value"] = json!("secret");
    cases.push(p);
    let mut p = point();
    p["timestamp"] = json!(chrono::Utc::now().timestamp() - 31 * 86400);
    cases.push(p);
    let mut p = point();
    p["trace_id"] = json!("not-an-id");
    cases.push(p);
    let mut p = point();
    p["value"] = json!(2);
    cases.push(p);
    let mut p = point();
    p["name"] = json!("unknown");
    cases.push(p);
    let mut p = point();
    p["trace_id"] = json!("00000000000000000000000000000000");
    cases.push(p);
    let mut p = point();
    p["timestamp"] = json!(chrono::Utc::now().timestamp() + 360);
    cases.push(p);
    cases.push(payment_point("lightning", "unknown"));
    cases.push(swap_point("bitcoin", "bitcoin"));
    cases.push(swap_point("bitcoin", "unknown"));
    let mut duration = point();
    duration["name"] = json!("bull.sync.duration");
    cases.push(duration);
    for p in cases {
        assert_eq!(
            f.send(envelope(vec![valid.clone(), p])).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let mixed = format!(
        "{}\n{{\"type\":\"event\"}}\n{{\"event_id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}",
        envelope(vec![valid]).replacen(
            "{}",
            "{\"event_id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}",
            1
        )
    );
    assert_eq!(f.send(mixed).await, StatusCode::UNPROCESSABLE_ENTITY);
    let c = f.counts();
    assert_eq!((c.archives, c.replays, c.samples), (0, 0, 0));
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn failed_aggregate_rolls_back_replay_marker() {
    let f = Fixture::new();
    assert_eq!(f.send(envelope(vec![point()])).await, StatusCode::OK);
    diesel::sql_query(
        "UPDATE metric_hour SET sample_count=9223372036854775807 WHERE project_id=$1",
    )
    .bind::<diesel::sql_types::Integer, _>(f.project)
    .execute(&mut f.pool.get().unwrap())
    .unwrap();
    let mut next = point();
    next["trace_id"] = json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    assert_eq!(
        f.send(envelope(vec![next])).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let c = f.counts();
    assert_eq!((c.archives, c.replays, c.samples), (0, 1, i64::MAX));
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn retention_removes_expired_buckets_and_replay_markers() {
    let f = Fixture::new();
    assert_eq!(f.send(envelope(vec![point()])).await, StatusCode::OK);
    let mut conn = f.pool.get().unwrap();
    diesel::sql_query("INSERT INTO metric_replay (project_id,digest,expires_at) VALUES ($1,'expired',(NOW() AT TIME ZONE 'UTC') - interval '1 second')")
        .bind::<diesel::sql_types::Integer,_>(f.project).execute(&mut conn).unwrap();
    diesel::sql_query("INSERT INTO metric_hour (project_id,bucket_start,name,release,environment,from_schema,to_schema,chain,outcome,sample_count,value_sum,value_min,value_max,histogram) SELECT project_id,bucket_start - interval '32 days',name,release,environment,from_schema,to_schema,chain,outcome,sample_count,value_sum,value_min,value_max,histogram FROM metric_hour WHERE project_id=$1")
        .bind::<diesel::sql_types::Integer,_>(f.project).execute(&mut conn).unwrap();
    drop(conn);
    Repositories::new(f.pool.clone())
        .analytics
        .cleanup_metrics()
        .unwrap();
    let c = f.counts();
    assert_eq!((c.archives, c.replays, c.samples), (0, 1, 1));
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn active_installations_accept_only_anonymous_unit_counts() {
    let f = Fixture::new();
    let active = point();

    let mut identified = active.clone();
    identified["attributes"]["installation_id"] = json!({"type":"string","value":"stable"});
    let mut invalid_value = active.clone();
    invalid_value["value"] = json!(2);
    for invalid in [identified, invalid_value] {
        let batch = envelope(vec![active.clone(), invalid]);
        assert_eq!(f.send(batch).await, StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(f.counts().samples, 0);

    let batch = envelope(vec![active]);
    assert_eq!(f.send(batch.clone()).await, StatusCode::OK);
    assert_eq!(f.send(batch).await, StatusCode::OK);
    let counts = f.counts();
    assert_eq!(
        (
            counts.archives,
            counts.replays,
            counts.samples,
            counts.total
        ),
        (0, 1, 1, 1.0)
    );
}

fn payment_point(network: &str, direction: &str) -> Value {
    let mut point = point();
    point["name"] = json!("bull.payment.completed");
    let attrs = point["attributes"].as_object_mut().unwrap();
    attrs.remove("from_schema");
    attrs.remove("to_schema");
    attrs.insert("network".into(), json!({"type":"string","value":network}));
    attrs.insert(
        "direction".into(),
        json!({"type":"string","value":direction}),
    );
    point
}

fn swap_point(source: &str, destination: &str) -> Value {
    let mut point = point();
    point["name"] = json!("bull.swap.completed");
    let attrs = point["attributes"].as_object_mut().unwrap();
    attrs.remove("from_schema");
    attrs.remove("to_schema");
    attrs.insert(
        "source_network".into(),
        json!({"type":"string","value":source}),
    );
    attrs.insert(
        "destination_network".into(),
        json!({"type":"string","value":destination}),
    );
    point
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn rejects_prototype_metrics_without_storing_any_batch_points() {
    let f = Fixture::new();
    let mut attempt = point();
    attempt["name"] = json!("bull.migration.attempt");
    attempt["attributes"]["from_schema"] = json!({"type":"integer","value":0});
    attempt["attributes"]["to_schema"] = json!({"type":"integer","value":1});
    let mut result = attempt.clone();
    result["name"] = json!("bull.migration.result");
    result["attributes"]["outcome"] = json!({"type":"string","value":"success"});
    for invalid in [
        attempt,
        result,
        payment_point("bitcoin", "send"),
        swap_point("bitcoin", "liquid"),
    ] {
        assert_eq!(
            f.send(envelope(vec![point(), invalid])).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let counts = f.counts();
    assert_eq!((counts.archives, counts.replays, counts.samples), (0, 0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
async fn hourly_series_cap_is_atomic_under_concurrent_requests() {
    let f = Arc::new(Fixture::new());
    let ts = chrono::Utc::now().timestamp();
    let make_point = |i: usize| {
        let mut p = point();
        p["timestamp"] = json!(ts);
        p["attributes"]["sentry.release"]["value"] = json!(format!("1.0.0+{i}"));
        p
    };
    let mut first_batch = String::new();
    for start in (0..1023).step_by(128) {
        let body = envelope((start..(start + 128).min(1023)).map(make_point).collect());
        if start == 0 {
            first_batch = body.clone();
        }
        assert_eq!(f.send(body).await, StatusCode::OK);
    }
    let request_a = envelope(vec![make_point(1023)]);
    let request_b = envelope(vec![make_point(1024)]);
    let first = {
        let f = f.clone();
        let body = request_a.clone();
        tokio::spawn(async move { f.send(body).await })
    };
    let second = {
        let f = f.clone();
        let body = request_b.clone();
        tokio::spawn(async move { f.send(body).await })
    };
    let (a, b) = tokio::join!(first, second);
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(
        (a == StatusCode::OK && b == StatusCode::UNPROCESSABLE_ENTITY)
            || (b == StatusCode::OK && a == StatusCode::UNPROCESSABLE_ENTITY),
        "{a} {b}"
    );
    let c = f.counts();
    assert_eq!((c.archives, c.replays, c.samples), (0, 9, 1024));
    assert_eq!(f.send(first_batch).await, StatusCode::OK);
    let mut existing_in_rejected_batch = make_point(0);
    existing_in_rejected_batch["trace_id"] = json!("cccccccccccccccccccccccccccccccc");
    let rejected = envelope(vec![existing_in_rejected_batch, make_point(1025)]);
    assert_eq!(f.send(rejected).await, StatusCode::UNPROCESSABLE_ENTITY);
    let c = f.counts();
    assert_eq!((c.archives, c.replays, c.samples), (0, 9, 1024));
    let mut existing = make_point(0);
    existing["trace_id"] = json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    assert_eq!(f.send(envelope(vec![existing])).await, StatusCode::OK);
    let c = f.counts();
    assert_eq!((c.archives, c.replays, c.samples), (0, 10, 1025));
    let mut prior_hour = make_point(1025);
    prior_hour["timestamp"] = json!(ts - 3600);
    assert_eq!(f.send(envelope(vec![prior_hour])).await, StatusCode::OK);
}
