use crash_cache::shared::{
    parser::SentrySession,
    persistence::{
        Repositories, db::models::NewSessionModel, establish_connection_pool, run_migrations,
    },
};
use std::sync::{Arc, Barrier};

#[test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL"]
fn concurrent_uuid_spellings_share_one_session_and_highest_sequence_wins() {
    let url = std::env::var("DATABASE_URL").expect("isolated DATABASE_URL required");
    assert!(url.contains("crash_cache_test"));
    let pool = establish_connection_pool(&url, 8, 10);
    run_migrations(&pool);
    let repos = Repositories::new(pool.clone());
    let project = repos.project.create(None, None).unwrap();
    let (ok, crashed, release) = {
        let mut conn = pool.get().unwrap();
        (
            repos.session_status.get_or_create(&mut conn, "ok").unwrap(),
            repos
                .session_status
                .get_or_create(&mut conn, "crashed")
                .unwrap(),
            repos
                .session_release
                .get_or_create(&mut conn, "test@1")
                .unwrap(),
        )
    };
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (1..=8).map(|seq| {
        let (repos, pool, barrier) = (repos.clone(), pool.clone(), barrier.clone());
        std::thread::spawn(move || {
            let spelling = if seq % 2 == 0 { "7C7B6585F9014351BF8D02711B721929" } else { "7c7b6585-f901-4351-bf8d-02711b721929" };
            let payload = serde_json::json!({"sid":spelling,"did":"same-installation","seq":seq,
                "started":"2026-09-26T10:00:00Z","timestamp":"2026-09-26T10:01:00Z",
                "errors":seq,"status":if seq == 8 {"crashed"} else {"ok"},"attrs":{"release":"test@1"}});
            let session = SentrySession::parse(&serde_json::to_vec(&payload).unwrap()).unwrap();
            let update = NewSessionModel {
                project_id:project, sid:session.sid.clone(), init:0, started_at:session.started.clone(),
                timestamp:session.timestamp.clone().unwrap(), errors:session.errors,
                status_id:if seq == 8 {crashed} else {ok}, release_id:Some(release), environment_id:None,
                distinct_id:session.distinct_id_hash(), sequence:session.sequence_decimal(), duration:None, abnormal_mechanism:None,
            };
            let mut conn = pool.get().unwrap();
            barrier.wait();
            repos.session.upsert(&mut conn, update).unwrap()
        })
    }).collect();
    let ids: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(ids.iter().all(|id| *id == ids[0]));
    assert_eq!(repos.session.count_by_project(project).unwrap(), 1);
    let session = repos
        .session
        .find_by_sid(project, "7c7b6585-f901-4351-bf8d-02711b721929")
        .unwrap()
        .unwrap();
    assert_eq!(session.sequence, "8");
    assert_eq!(session.errors, 8);
    assert_eq!(session.status_id, crashed);
}
