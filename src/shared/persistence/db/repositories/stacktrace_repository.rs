use diesel::prelude::*;
use super::DbPool;


use crate::shared::persistence::db::models::{NewUnwrapStacktraceModel, UnwrapStacktraceModel};
use crate::shared::persistence::db::schema::unwrap_stacktrace;


#[derive(Clone)]
pub struct StacktraceRepository {
    pool: DbPool,
}

impl StacktraceRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn get_or_create(
        &self,
        hash: &str,
        fingerprint_hash: Option<String>,
        frames_json: &str,
    ) -> Result<i32, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        if let Some(existing) = unwrap_stacktrace::table
            .filter(unwrap_stacktrace::hash.eq(hash))
            .select(UnwrapStacktraceModel::as_select())
            .first::<UnwrapStacktraceModel>(&mut conn)
            .optional()?
        {
            return Ok(existing.id);
        }

        let new_record = NewUnwrapStacktraceModel {
            hash: hash.to_string(),
            fingerprint_hash,
            frames_json: frames_json.to_string(),
        };

        diesel::insert_into(unwrap_stacktrace::table)
            .values(&new_record)
            .execute(&mut conn)?;

        let inserted = unwrap_stacktrace::table
            .filter(unwrap_stacktrace::hash.eq(hash))
            .select(UnwrapStacktraceModel::as_select())
            .first::<UnwrapStacktraceModel>(&mut conn)?;

        Ok(inserted.id)
    }

    pub fn find_by_hash(
        &self,
        hash: &str,
    ) -> Result<Option<UnwrapStacktraceModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        unwrap_stacktrace::table
            .filter(unwrap_stacktrace::hash.eq(hash))
            .select(UnwrapStacktraceModel::as_select())
            .first::<UnwrapStacktraceModel>(&mut conn)
            .optional()
    }

    pub fn find_by_fingerprint(
        &self,
        fingerprint_hash: &str,
    ) -> Result<Vec<UnwrapStacktraceModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        unwrap_stacktrace::table
            .filter(unwrap_stacktrace::fingerprint_hash.eq(fingerprint_hash))
            .select(UnwrapStacktraceModel::as_select())
            .load::<UnwrapStacktraceModel>(&mut conn)
    }
}
