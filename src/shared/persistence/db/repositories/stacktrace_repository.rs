use super::{DbConnection, DbPool};
use crate::shared::domain::DomainError;
use crate::shared::persistence::db::models::{NewUnwrapStacktraceModel, UnwrapStacktraceModel};
use crate::shared::persistence::db::schema::unwrap_stacktrace;
use diesel::prelude::*;

#[derive(Clone)]
pub struct StacktraceRepository {}

impl StacktraceRepository {
    pub fn new(_pool: DbPool) -> Self {
        Self {}
    }

    pub fn get_or_create(
        &self,
        conn: &mut DbConnection,
        hash: &str,
        fingerprint_hash: Option<String>,
        frames: serde_json::Value,
    ) -> Result<i32, DomainError> {
        if let Some(existing) = unwrap_stacktrace::table
            .filter(unwrap_stacktrace::hash.eq(hash))
            .select(UnwrapStacktraceModel::as_select())
            .first::<UnwrapStacktraceModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?
        {
            return Ok(existing.id);
        }

        let new_record = NewUnwrapStacktraceModel {
            hash: hash.to_string(),
            fingerprint_hash,
            frames,
        };

        let id = diesel::insert_into(unwrap_stacktrace::table)
            .values(&new_record)
            .returning(unwrap_stacktrace::id)
            .get_result::<i32>(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn find_by_hash(
        &self,
        conn: &mut DbConnection,
        hash: &str,
    ) -> Result<Option<UnwrapStacktraceModel>, DomainError> {
        unwrap_stacktrace::table
            .filter(unwrap_stacktrace::hash.eq(hash))
            .select(UnwrapStacktraceModel::as_select())
            .first::<UnwrapStacktraceModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))
    }

    pub fn find_by_fingerprint(
        &self,
        conn: &mut DbConnection,
        fingerprint_hash: &str,
    ) -> Result<Vec<UnwrapStacktraceModel>, DomainError> {
        unwrap_stacktrace::table
            .filter(unwrap_stacktrace::fingerprint_hash.eq(fingerprint_hash))
            .select(UnwrapStacktraceModel::as_select())
            .load::<UnwrapStacktraceModel>(conn)
            .map_err(|e| DomainError::Database(e.to_string()))
    }
}
