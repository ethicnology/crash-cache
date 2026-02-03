use diesel::prelude::*;
use super::DbPool;


use crate::shared::persistence::db::models::{UnwrapExceptionMessageModel, NewUnwrapExceptionMessageModel};
use crate::shared::persistence::db::schema::unwrap_exception_message;


#[derive(Clone)]
pub struct ExceptionMessageRepository {
    pool: DbPool,
}

impl ExceptionMessageRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn get_or_create(&self, hash: &str, value: &str) -> Result<i32, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        if let Some(existing) = unwrap_exception_message::table
            .filter(unwrap_exception_message::hash.eq(hash))
            .select(UnwrapExceptionMessageModel::as_select())
            .first::<UnwrapExceptionMessageModel>(&mut conn)
            .optional()?
        {
            return Ok(existing.id);
        }

        let new_record = NewUnwrapExceptionMessageModel {
            hash: hash.to_string(),
            value: value.to_string(),
        };

        diesel::insert_into(unwrap_exception_message::table)
            .values(&new_record)
            .execute(&mut conn)?;

        let inserted = unwrap_exception_message::table
            .filter(unwrap_exception_message::hash.eq(hash))
            .select(UnwrapExceptionMessageModel::as_select())
            .first::<UnwrapExceptionMessageModel>(&mut conn)?;

        Ok(inserted.id)
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<UnwrapExceptionMessageModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        unwrap_exception_message::table
            .filter(unwrap_exception_message::hash.eq(hash))
            .select(UnwrapExceptionMessageModel::as_select())
            .first::<UnwrapExceptionMessageModel>(&mut conn)
            .optional()
    }
}
