use super::DbConnection;
use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{Archive, DomainError};
use crate::shared::persistence::db::models::ArchiveModel;
use crate::shared::persistence::db::schema::archive;

#[derive(Clone, Default)]
pub struct ArchiveRepository {}

impl ArchiveRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn save(&self, conn: &mut DbConnection, arch: &Archive) -> Result<(), DomainError> {
        let model = ArchiveModel {
            hash: arch.hash.clone(),
            project_id: arch.project_id,
            compressed_payload: arch.compressed_payload.clone(),
            original_size: arch.original_size,
            created_at: arch.created_at.naive_utc(),
        };

        diesel::insert_into(archive::table)
            .values(&model)
            .on_conflict(archive::hash)
            .do_nothing()
            .execute(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn find_by_hash(
        &self,
        conn: &mut DbConnection,
        hash: &str,
    ) -> Result<Option<Archive>, DomainError> {
        let result = archive::table
            .filter(archive::hash.eq(hash))
            .first::<ArchiveModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| Archive {
            hash: m.hash,
            project_id: m.project_id,
            compressed_payload: m.compressed_payload,
            original_size: m.original_size,
            created_at: Utc.from_utc_datetime(&m.created_at),
        }))
    }

    pub fn exists(&self, conn: &mut DbConnection, hash: &str) -> Result<bool, DomainError> {
        let count: i64 = archive::table
            .filter(archive::hash.eq(hash))
            .count()
            .get_result(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count > 0)
    }
}
