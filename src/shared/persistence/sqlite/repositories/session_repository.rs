use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;

use crate::shared::persistence::sqlite::models::*;
use crate::shared::persistence::sqlite::schema::*;

type SqlitePool = Pool<ConnectionManager<SqliteConnection>>;

// ============================================
// SESSION UNWRAP REPOSITORIES
// ============================================

macro_rules! impl_session_unwrap_repository {
    ($repo_name:ident, $table:ident, $model:ident, $new_model:ident) => {
        #[derive(Clone)]
        pub struct $repo_name {
            pool: SqlitePool,
        }

        impl $repo_name {
            pub fn new(pool: SqlitePool) -> Self {
                Self { pool }
            }

            pub fn get_or_create(&self, val: &str) -> Result<i32, diesel::result::Error> {
                let mut conn = self.pool.get().expect("Failed to get connection");

                if let Some(existing) = $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .optional()?
                {
                    return Ok(existing.id);
                }

                let new_record = $new_model {
                    value: val.to_string(),
                };

                diesel::insert_into($table::table)
                    .values(&new_record)
                    .execute(&mut conn)?;

                let inserted = $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)?;

                Ok(inserted.id)
            }

            pub fn find_by_id(&self, id: i32) -> Result<Option<$model>, diesel::result::Error> {
                let mut conn = self.pool.get().expect("Failed to get connection");

                $table::table
                    .filter($table::id.eq(id))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .optional()
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
    pool: SqlitePool,
}

impl SessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Creates or updates a session. Uses INSERT OR REPLACE on (project_id, sid).
    /// Returns the session ID.
    pub fn upsert(&self, new_session: NewSessionModel) -> Result<i32, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        // Check if session already exists
        if let Some(existing) = session::table
            .filter(session::project_id.eq(new_session.project_id))
            .filter(session::sid.eq(&new_session.sid))
            .select(SessionModel::as_select())
            .first::<SessionModel>(&mut conn)
            .optional()?
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
                .execute(&mut conn)?;

            return Ok(existing.id);
        }

        // Insert new session
        diesel::insert_into(session::table)
            .values(&new_session)
            .execute(&mut conn)?;

        let inserted = session::table
            .filter(session::project_id.eq(new_session.project_id))
            .filter(session::sid.eq(&new_session.sid))
            .select(SessionModel::as_select())
            .first::<SessionModel>(&mut conn)?;

        Ok(inserted.id)
    }

    pub fn find_by_sid(&self, project_id: i32, sid: &str) -> Result<Option<SessionModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        session::table
            .filter(session::project_id.eq(project_id))
            .filter(session::sid.eq(sid))
            .select(SessionModel::as_select())
            .first::<SessionModel>(&mut conn)
            .optional()
    }

    pub fn count_by_project(&self, project_id: i32) -> Result<i64, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        session::table
            .filter(session::project_id.eq(project_id))
            .count()
            .get_result(&mut conn)
    }

    pub fn count_by_status(&self, project_id: i32, status_id: i32) -> Result<i64, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        session::table
            .filter(session::project_id.eq(project_id))
            .filter(session::status_id.eq(status_id))
            .count()
            .get_result(&mut conn)
    }
}
