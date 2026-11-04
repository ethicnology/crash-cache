use super::{DbConnection, DbPool};
use crate::shared::domain::DomainError;
use crate::shared::persistence::db::models::*;
use crate::shared::persistence::db::schema::*;
use diesel::prelude::*;

// ============================================
// SESSION UNWRAP REPOSITORIES
// ============================================

macro_rules! impl_session_unwrap_repository {
    ($repo_name:ident, $table:ident, $model:ident, $new_model:ident) => {
        #[derive(Clone)]
        pub struct $repo_name {}

        impl $repo_name {
            pub fn new(_pool: DbPool) -> Self {
                Self {}
            }

            pub fn get_or_create(
                &self,
                conn: &mut DbConnection,
                val: &str,
            ) -> Result<i32, DomainError> {
                if let Some(existing) = $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(conn)
                    .optional()
                    .map_err(|e| DomainError::Database(e.to_string()))?
                {
                    return Ok(existing.id);
                }

                let new_record = $new_model {
                    value: val.to_string(),
                };

                let id = diesel::insert_into($table::table)
                    .values(&new_record)
                    .on_conflict($table::value)
                    .do_update()
                    .set($table::value.eq(val))
                    .returning($table::id)
                    .get_result::<i32>(conn)
                    .map_err(|e| DomainError::Database(e.to_string()))?;

                Ok(id)
            }

            pub fn find_by_id(
                &self,
                conn: &mut DbConnection,
                id: i32,
            ) -> Result<Option<$model>, DomainError> {
                $table::table
                    .filter($table::id.eq(id))
                    .select($model::as_select())
                    .first::<$model>(conn)
                    .optional()
                    .map_err(|e| DomainError::Database(e.to_string()))
            }
        }
    };
}

impl_session_unwrap_repository!(
    UnwrapSessionStatusRepository,
    unwrap_session_status,
    UnwrapSessionStatusModel,
    NewUnwrapSessionStatusModel
);

impl_session_unwrap_repository!(
    UnwrapSessionReleaseRepository,
    unwrap_session_release,
    UnwrapSessionReleaseModel,
    NewUnwrapSessionReleaseModel
);

impl_session_unwrap_repository!(
    UnwrapSessionEnvironmentRepository,
    unwrap_session_environment,
    UnwrapSessionEnvironmentModel,
    NewUnwrapSessionEnvironmentModel
);

// ============================================
// SESSION REPOSITORY
// ============================================

#[derive(Clone)]
pub struct SessionRepository {
    pool: DbPool,
}

impl SessionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Atomically merges a complete update, retaining only the latest logical state.
    /// Retries and delayed initial updates cannot reopen or duplicate a session.
    pub fn upsert(
        &self,
        conn: &mut DbConnection,
        new_session: NewSessionModel,
    ) -> Result<i32, DomainError> {
        use diesel::sql_types::{Double, Integer, Nullable, Text};
        #[derive(QueryableByName)]
        struct SessionId {
            #[diesel(sql_type = Integer)]
            id: i32,
        }
        // A no-op conflict update still returns the existing identifier atomically.
        // Numeric casts preserve Sentry's full u64 sequence range without overflow.
        diesel::sql_query(
            "INSERT INTO session (project_id, sid, init, started_at, timestamp, errors,
                 status_id, release_id, environment_id, distinct_id, sequence, duration, abnormal_mechanism)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
             ON CONFLICT (project_id, sid) DO UPDATE SET
                 init = GREATEST(session.init, EXCLUDED.init),
                 timestamp = CASE WHEN EXCLUDED.sequence::numeric > session.sequence::numeric THEN EXCLUDED.timestamp ELSE session.timestamp END,
                 errors = CASE WHEN EXCLUDED.sequence::numeric > session.sequence::numeric THEN EXCLUDED.errors ELSE session.errors END,
                 status_id = CASE WHEN EXCLUDED.sequence::numeric > session.sequence::numeric THEN EXCLUDED.status_id ELSE session.status_id END,
                 duration = CASE WHEN EXCLUDED.sequence::numeric > session.sequence::numeric THEN EXCLUDED.duration ELSE session.duration END,
                 abnormal_mechanism = CASE WHEN EXCLUDED.sequence::numeric > session.sequence::numeric THEN EXCLUDED.abnormal_mechanism ELSE session.abnormal_mechanism END,
                 sequence = CASE WHEN EXCLUDED.sequence::numeric > session.sequence::numeric THEN EXCLUDED.sequence ELSE session.sequence END
             RETURNING id"
        )
        .bind::<Integer, _>(new_session.project_id)
        .bind::<Text, _>(new_session.sid)
        .bind::<Integer, _>(new_session.init)
        .bind::<Text, _>(new_session.started_at)
        .bind::<Text, _>(new_session.timestamp)
        .bind::<Integer, _>(new_session.errors)
        .bind::<Integer, _>(new_session.status_id)
        .bind::<Nullable<Integer>, _>(new_session.release_id)
        .bind::<Nullable<Integer>, _>(new_session.environment_id)
        .bind::<Nullable<Text>, _>(new_session.distinct_id)
        .bind::<Text, _>(new_session.sequence)
        .bind::<Nullable<Double>, _>(new_session.duration)
        .bind::<Nullable<Text>, _>(new_session.abnormal_mechanism)
        .get_result::<SessionId>(conn)
        .map(|row| row.id)
        .map_err(|error| DomainError::Database(error.to_string()))
    }

    pub fn find_by_sid(
        &self,
        project_id: i32,
        sid: &str,
    ) -> Result<Option<SessionModel>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.find_by_sid_with_conn(&mut conn, project_id, sid)
    }

    pub fn find_by_sid_with_conn(
        &self,
        conn: &mut DbConnection,
        project_id: i32,
        sid: &str,
    ) -> Result<Option<SessionModel>, DomainError> {
        session::table
            .filter(session::project_id.eq(project_id))
            .filter(session::sid.eq(sid))
            .select(SessionModel::as_select())
            .first::<SessionModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))
    }

    pub fn count_by_project(&self, project_id: i32) -> Result<i64, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.count_by_project_with_conn(&mut conn, project_id)
    }

    pub fn count_by_project_with_conn(
        &self,
        conn: &mut DbConnection,
        project_id: i32,
    ) -> Result<i64, DomainError> {
        session::table
            .filter(session::project_id.eq(project_id))
            .count()
            .get_result(conn)
            .map_err(|e| DomainError::Database(e.to_string()))
    }

    pub fn count_by_status(&self, project_id: i32, status_id: i32) -> Result<i64, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.count_by_status_with_conn(&mut conn, project_id, status_id)
    }

    pub fn count_by_status_with_conn(
        &self,
        conn: &mut DbConnection,
        project_id: i32,
        status_id: i32,
    ) -> Result<i64, DomainError> {
        session::table
            .filter(session::project_id.eq(project_id))
            .filter(session::status_id.eq(status_id))
            .count()
            .get_result(conn)
            .map_err(|e| DomainError::Database(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::persistence::{establish_connection_pool, run_migrations, Repositories};

    #[test]
    #[ignore = "requires isolated PostgreSQL DATABASE_URL"]
    fn session_retry_cannot_reopen_a_crashed_session() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
        let pool = establish_connection_pool(&url, 2, 10);
        run_migrations(&pool);
        let repos = Repositories::new(pool.clone());
        let project_id = repos.project.create(None, None).unwrap();
        let mut conn = pool.get().unwrap();
        let ok = repos.session_status.get_or_create(&mut conn, "ok").unwrap();
        let crashed = repos.session_status.get_or_create(&mut conn, "crashed").unwrap();
        let make_update = |status_id, init, timestamp: &str, errors| NewSessionModel {
            project_id,
            sid: "7c7b6585-f901-4351-bf8d-02711b721929".into(),
            init,
            started_at: "2026-09-26T10:00:00Z".into(),
            timestamp: timestamp.into(),
            errors,
            status_id,
            release_id: None,
            environment_id: None,
            distinct_id: Some("installation-hash".into()),
            sequence: if init == 1 { "0".into() } else { "100".into() },
            duration: Some(60.0),
            abnormal_mechanism: None,
        };
        let id = repos.session.upsert(&mut conn, make_update(crashed, 0, "2026-09-26T10:01:00Z", 1)).unwrap();
        let retry = repos.session.upsert(&mut conn, make_update(ok, 1, "2026-09-26T10:00:00Z", 0)).unwrap();
        assert_eq!(id, retry);
        let row = repos.session.find_by_sid_with_conn(&mut conn, project_id, "7c7b6585-f901-4351-bf8d-02711b721929").unwrap().unwrap();
        assert_eq!(row.status_id, crashed);
        assert_eq!(row.errors, 1);
        assert_eq!(row.init, 1);
        assert_eq!(row.sequence, "100");
        assert_eq!(row.distinct_id.as_deref(), Some("installation-hash"));
        assert_eq!(row.duration, Some(60.0));
        // The logical sequence wins even when wall-clock timestamps disagree.
        let mut newest = make_update(crashed, 0, "2026-09-26T10:00:30Z", 3);
        newest.sequence = u64::MAX.to_string();
        newest.distinct_id = Some("invalid-identity-change".into());
        repos.session.upsert(&mut conn, newest).unwrap();
        let mut duplicate = make_update(ok, 0, "2026-09-26T11:00:00Z", 0);
        duplicate.sequence = u64::MAX.to_string();
        repos.session.upsert(&mut conn, duplicate).unwrap();
        let row = repos.session.find_by_sid_with_conn(&mut conn, project_id, "7c7b6585-f901-4351-bf8d-02711b721929").unwrap().unwrap();
        assert_eq!(row.status_id, crashed);
        assert_eq!(row.errors, 3);
        assert_eq!(row.timestamp, "2026-09-26T10:00:30Z");
        assert_eq!(row.distinct_id.as_deref(), Some("installation-hash"));
        assert_eq!(repos.session.count_by_project_with_conn(&mut conn, project_id).unwrap(), 1);
    }
}
