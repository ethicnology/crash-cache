use super::{DbConnection, DbPool};
use crate::shared::domain::DomainError;
use crate::shared::persistence::db::models::{NewUnwrapStacktraceModel, UnwrapStacktraceModel};
use crate::shared::persistence::db::schema::unwrap_stacktrace;
use diesel::prelude::*;

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
    ) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.get_or_create_with_conn(&mut conn, hash, fingerprint_hash, frames_json)
    }

    pub fn get_or_create_with_conn(
        &self,
        conn: &mut DbConnection,
        hash: &str,
        fingerprint_hash: Option<String>,
        frames_json: &str,
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
            frames_json: frames_json.to_string(),
        };

        let id = diesel::insert_into(unwrap_stacktrace::table)
            .values(&new_record)
            .returning(unwrap_stacktrace::id)
            .get_result::<i32>(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<UnwrapStacktraceModel>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.find_by_hash_with_conn(&mut conn, hash)
    }

    pub fn find_by_hash_with_conn(
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
        fingerprint_hash: &str,
    ) -> Result<Vec<UnwrapStacktraceModel>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.find_by_fingerprint_with_conn(&mut conn, fingerprint_hash)
    }

    pub fn find_by_fingerprint_with_conn(
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
