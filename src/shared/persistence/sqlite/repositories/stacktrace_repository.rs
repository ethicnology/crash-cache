use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;

use crate::shared::persistence::sqlite::models::{NewStacktraceModel, StacktraceModel};
use crate::shared::persistence::sqlite::schema::stacktrace;

type SqlitePool = Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone)]
pub struct StacktraceRepository {
    pool: SqlitePool,
}

impl StacktraceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn get_or_create(
        &self,
        hash: &str,
        issue_id: Option<i32>,
        frames_json: &[u8],
    ) -> Result<i32, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        if let Some(existing) = stacktrace::table
            .filter(stacktrace::hash.eq(hash))
            .select(StacktraceModel::as_select())
            .first::<StacktraceModel>(&mut conn)
            .optional()?
        {
            return Ok(existing.id);
        }

        let new_record = NewStacktraceModel {
            hash: hash.to_string(),
            issue_id,
            frames_json: frames_json.to_vec(),
        };

        diesel::insert_into(stacktrace::table)
            .values(&new_record)
            .execute(&mut conn)?;

        let inserted = stacktrace::table
            .filter(stacktrace::hash.eq(hash))
            .select(StacktraceModel::as_select())
            .first::<StacktraceModel>(&mut conn)?;

        Ok(inserted.id)
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<StacktraceModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        stacktrace::table
            .filter(stacktrace::hash.eq(hash))
            .select(StacktraceModel::as_select())
            .first::<StacktraceModel>(&mut conn)
            .optional()
    }

    pub fn find_by_issue_id(&self, issue_id: i32) -> Result<Vec<StacktraceModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        stacktrace::table
            .filter(stacktrace::issue_id.eq(issue_id))
            .select(StacktraceModel::as_select())
            .load::<StacktraceModel>(&mut conn)
    }
}
