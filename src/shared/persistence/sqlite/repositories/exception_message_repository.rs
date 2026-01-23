use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;

use crate::shared::persistence::sqlite::models::{ExceptionMessageModel, NewExceptionMessageModel};
use crate::shared::persistence::sqlite::schema::exception_message;

type SqlitePool = Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone)]
pub struct ExceptionMessageRepository {
    pool: SqlitePool,
}

impl ExceptionMessageRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn get_or_create(&self, hash: &str, value: &str) -> Result<i32, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        if let Some(existing) = exception_message::table
            .filter(exception_message::hash.eq(hash))
            .select(ExceptionMessageModel::as_select())
            .first::<ExceptionMessageModel>(&mut conn)
            .optional()?
        {
            return Ok(existing.id);
        }

        let new_record = NewExceptionMessageModel {
            hash: hash.to_string(),
            value: value.to_string(),
        };

        diesel::insert_into(exception_message::table)
            .values(&new_record)
            .execute(&mut conn)?;

        let inserted = exception_message::table
            .filter(exception_message::hash.eq(hash))
            .select(ExceptionMessageModel::as_select())
            .first::<ExceptionMessageModel>(&mut conn)?;

        Ok(inserted.id)
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<ExceptionMessageModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        exception_message::table
            .filter(exception_message::hash.eq(hash))
            .select(ExceptionMessageModel::as_select())
            .first::<ExceptionMessageModel>(&mut conn)
            .optional()
    }
}
