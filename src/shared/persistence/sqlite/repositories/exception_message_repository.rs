use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;

use crate::shared::persistence::sqlite::models::{LookupExceptionMessageModel, NewLookupExceptionMessageModel};
use crate::shared::persistence::sqlite::schema::lookup_exception_message;

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

        if let Some(existing) = lookup_exception_message::table
            .filter(lookup_exception_message::hash.eq(hash))
            .select(LookupExceptionMessageModel::as_select())
            .first::<LookupExceptionMessageModel>(&mut conn)
            .optional()?
        {
            return Ok(existing.id);
        }

        let new_record = NewLookupExceptionMessageModel {
            hash: hash.to_string(),
            value: value.to_string(),
        };

        diesel::insert_into(lookup_exception_message::table)
            .values(&new_record)
            .execute(&mut conn)?;

        let inserted = lookup_exception_message::table
            .filter(lookup_exception_message::hash.eq(hash))
            .select(LookupExceptionMessageModel::as_select())
            .first::<LookupExceptionMessageModel>(&mut conn)?;

        Ok(inserted.id)
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<LookupExceptionMessageModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        lookup_exception_message::table
            .filter(lookup_exception_message::hash.eq(hash))
            .select(LookupExceptionMessageModel::as_select())
            .first::<LookupExceptionMessageModel>(&mut conn)
            .optional()
    }
}
