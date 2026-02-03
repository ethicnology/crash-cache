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
        pub struct $repo_name {
            pool: DbPool,
        }

        impl $repo_name {
            pub fn new(pool: DbPool) -> Self {
                Self { pool }
            }

            pub fn get_or_create(&self, val: &str) -> Result<i32, DomainError> {
                let mut conn = self.pool.get().map_err(|e| {
                    DomainError::ConnectionPool(format!("Connection pool error: {}", e))
                })?;
                self.get_or_create_with_conn(&mut conn, val)
            }

            pub fn get_or_create_with_conn(
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
                    .returning($table::id)
                    .get_result::<i32>(conn)
                    .map_err(|e| DomainError::Database(e.to_string()))?;

                Ok(id)
            }

            pub fn find_by_id(&self, id: i32) -> Result<Option<$model>, DomainError> {
                let mut conn = self.pool.get().map_err(|e| {
                    DomainError::ConnectionPool(format!("Connection pool error: {}", e))
                })?;
                self.find_by_id_with_conn(&mut conn, id)
            }

            pub fn find_by_id_with_conn(
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

    /// Creates or updates a session. Uses INSERT OR REPLACE on (project_id, sid).
    /// Returns the session ID.
    pub fn upsert(&self, new_session: NewSessionModel) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.upsert_with_conn(&mut conn, new_session)
    }

    pub fn upsert_with_conn(
        &self,
        conn: &mut DbConnection,
        new_session: NewSessionModel,
    ) -> Result<i32, DomainError> {
        // Check if session already exists
        if let Some(existing) = session::table
            .filter(session::project_id.eq(new_session.project_id))
            .filter(session::sid.eq(&new_session.sid))
            .select(SessionModel::as_select())
            .first::<SessionModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?
        {
            // Update existing session
            diesel::update(session::table.filter(session::id.eq(existing.id)))
                .set((
                    session::init.eq(new_session.init),
                    session::started_at.eq(&new_session.started_at),
                    session::timestamp.eq(&new_session.timestamp),
                    session::errors.eq(new_session.errors),
                    session::status_id.eq(new_session.status_id),
                    session::release_id.eq(new_session.release_id),
                    session::environment_id.eq(new_session.environment_id),
                ))
                .execute(conn)
                .map_err(|e| DomainError::Database(e.to_string()))?;

            return Ok(existing.id);
        }

        // Insert new session
        let id = diesel::insert_into(session::table)
            .values(&new_session)
            .returning(session::id)
            .get_result::<i32>(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
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
