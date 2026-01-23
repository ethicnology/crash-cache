use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{Archive, DomainError};
use crate::shared::persistence::sqlite::models::ArchiveModel;
use crate::shared::persistence::sqlite::schema::archive;
use crate::shared::persistence::SqlitePool;

#[derive(Clone)]
pub struct ArchiveRepository {
    pool: SqlitePool,
}

impl ArchiveRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn save(&self, arch: &Archive) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let model = ArchiveModel {
            hash: arch.hash.clone(),
            compressed_payload: arch.compressed_payload.clone(),
            original_size: arch.original_size,
            created_at: arch.created_at.naive_utc(),
        };

        diesel::insert_or_ignore_into(archive::table)
            .values(&model)
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<Archive>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let result = archive::table
            .filter(archive::hash.eq(hash))
            .first::<ArchiveModel>(&mut conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| Archive {
            hash: m.hash,
            compressed_payload: m.compressed_payload,
            original_size: m.original_size,
            created_at: Utc.from_utc_datetime(&m.created_at),
        }))
    }

    pub fn exists(&self, hash: &str) -> Result<bool, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let count: i64 = archive::table
            .filter(archive::hash.eq(hash))
            .count()
            .get_result(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count > 0)
    }
}
