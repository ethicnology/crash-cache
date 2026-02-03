use super::{DbConnection, DbPool};
use crate::shared::domain::DomainError;
use crate::shared::persistence::db::models::{
    NewUnwrapExceptionMessageModel, UnwrapExceptionMessageModel,
};
use crate::shared::persistence::db::schema::unwrap_exception_message;
use diesel::prelude::*;

#[derive(Clone)]
pub struct ExceptionMessageRepository {
    pool: DbPool,
}

impl ExceptionMessageRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn get_or_create(&self, hash: &str, value: &str) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.get_or_create_with_conn(&mut conn, hash, value)
    }

    pub fn get_or_create_with_conn(
        &self,
        conn: &mut DbConnection,
        hash: &str,
        value: &str,
    ) -> Result<i32, DomainError> {
        if let Some(existing) = unwrap_exception_message::table
            .filter(unwrap_exception_message::hash.eq(hash))
            .select(UnwrapExceptionMessageModel::as_select())
            .first::<UnwrapExceptionMessageModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?
        {
            return Ok(existing.id);
        }

        let new_record = NewUnwrapExceptionMessageModel {
            hash: hash.to_string(),
            value: value.to_string(),
        };

        let id = diesel::insert_into(unwrap_exception_message::table)
            .values(&new_record)
            .returning(unwrap_exception_message::id)
            .get_result::<i32>(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn find_by_hash(
        &self,
        hash: &str,
    ) -> Result<Option<UnwrapExceptionMessageModel>, DomainError> {
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
    ) -> Result<Option<UnwrapExceptionMessageModel>, DomainError> {
        unwrap_exception_message::table
            .filter(unwrap_exception_message::hash.eq(hash))
            .select(UnwrapExceptionMessageModel::as_select())
            .first::<UnwrapExceptionMessageModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))
    }
}
