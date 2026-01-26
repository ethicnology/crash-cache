use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;

use crate::shared::persistence::sqlite::models::{NewLookupStacktraceModel, LookupStacktraceModel};
use crate::shared::persistence::sqlite::schema::lookup_stacktrace;

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
        fingerprint_hash: Option<String>,
        frames_json: &[u8],
    ) -> Result<i32, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        if let Some(existing) = lookup_stacktrace::table
            .filter(lookup_stacktrace::hash.eq(hash))
            .select(LookupStacktraceModel::as_select())
            .first::<LookupStacktraceModel>(&mut conn)
            .optional()?
        {
            return Ok(existing.id);
        }

        let new_record = NewLookupStacktraceModel {
            hash: hash.to_string(),
            fingerprint_hash,
            frames_json: frames_json.to_vec(),
        };

        diesel::insert_into(lookup_stacktrace::table)
            .values(&new_record)
            .execute(&mut conn)?;

        let inserted = lookup_stacktrace::table
            .filter(lookup_stacktrace::hash.eq(hash))
            .select(LookupStacktraceModel::as_select())
            .first::<LookupStacktraceModel>(&mut conn)?;

        Ok(inserted.id)
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<LookupStacktraceModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        lookup_stacktrace::table
            .filter(lookup_stacktrace::hash.eq(hash))
            .select(LookupStacktraceModel::as_select())
            .first::<LookupStacktraceModel>(&mut conn)
            .optional()
    }

    pub fn find_by_fingerprint(&self, fingerprint_hash: &str) -> Result<Vec<LookupStacktraceModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        lookup_stacktrace::table
            .filter(lookup_stacktrace::fingerprint_hash.eq(fingerprint_hash))
            .select(LookupStacktraceModel::as_select())
            .load::<LookupStacktraceModel>(&mut conn)
    }
}
