use crash_cache::shared::persistence::{establish_connection_pool, SessionRepository};
use crash_cache::shared::persistence::db::models::NewSessionModel;
use diesel::{connection::SimpleConnection, prelude::*};

#[test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL with CREATE SCHEMA"]
fn migrated_terminal_session_cannot_be_reopened_by_an_older_update() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = establish_connection_pool(&url, 2, 10);
    let repository = SessionRepository::new(pool.clone());
    let mut conn = pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        conn.batch_execute("CREATE SCHEMA session_migration_regression; SET LOCAL search_path TO session_migration_regression;")?;
        conn.batch_execute(include_str!("../migrations/00000000000000_initial_schema/up.sql"))?;
        conn.batch_execute("INSERT INTO project(id, created_at) VALUES (1, CURRENT_TIMESTAMP);
            INSERT INTO unwrap_session_status(id, value) VALUES (1, 'ok'), (2, 'crashed');
            INSERT INTO session(project_id,sid,init,started_at,timestamp,errors,status_id)
            VALUES (1,'7c7b6585-f901-4351-bf8d-02711b721929',0,'2026-09-26T10:00:00Z','2026-09-26T10:10:00Z',1,2);")?;
        conn.batch_execute(include_str!("../migrations/20260926000000_sentry_identity_sessions/up.sql"))?;
        repository.upsert(conn, NewSessionModel {
            project_id: 1,
            sid: "7c7b6585-f901-4351-bf8d-02711b721929".into(),
            init: 0,
            started_at: "2026-09-26T10:00:00Z".into(),
            timestamp: "2026-09-26T10:01:00Z".into(),
            errors: 0,
            status_id: 1,
            release_id: None,
            environment_id: None,
            distinct_id: None,
            sequence: chrono::DateTime::parse_from_rfc3339("2026-09-26T10:01:00Z").unwrap().timestamp_millis().to_string(),
            duration: None,
            abnormal_mechanism: None,
        }).unwrap();
        let row = repository.find_by_sid_with_conn(conn, 1, "7c7b6585-f901-4351-bf8d-02711b721929").unwrap().unwrap();
        assert_eq!(row.status_id, 2, "migration must preserve ordering of already stored session updates");
        assert_eq!(row.errors, 1);
        Ok(())
    });
}


#[test]
#[ignore = "requires isolated PostgreSQL DATABASE_URL with CREATE SCHEMA"]
fn migration_refuses_malformed_legacy_clock_instead_of_resetting_it() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = establish_connection_pool(&url, 1, 10);
    let mut conn = pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        conn.batch_execute("CREATE SCHEMA session_migration_invalid; SET LOCAL search_path TO session_migration_invalid;")?;
        conn.batch_execute(include_str!("../migrations/00000000000000_initial_schema/up.sql"))?;
        conn.batch_execute("INSERT INTO project(id, created_at) VALUES (1, CURRENT_TIMESTAMP);
            INSERT INTO unwrap_session_status(id, value) VALUES (1, 'crashed');
            INSERT INTO session(project_id,sid,init,started_at,timestamp,errors,status_id)
            VALUES (1,'7c7b6585-f901-4351-bf8d-02711b721929',0,'2026-09-26T10:00:00Z','invalid-clock',1,1);")?;
        let error = conn.batch_execute(include_str!("../migrations/20260926000000_sentry_identity_sessions/up.sql")).unwrap_err();
        assert!(error.to_string().contains("legacy update timestamp is not RFC3339"));
        Ok(())
    });
}
