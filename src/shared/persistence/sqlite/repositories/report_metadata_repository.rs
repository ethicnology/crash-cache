use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{DomainError, ReportMetadata};
use crate::shared::persistence::sqlite::models::{NewReportMetadataModel, ReportMetadataModel};
use crate::shared::persistence::sqlite::schema::report_metadata;
use crate::shared::persistence::SqlitePool;

#[derive(Clone)]
pub struct ReportMetadataRepository {
    pool: SqlitePool,
}

impl ReportMetadataRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn save(&self, metadata: &ReportMetadata) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let model = NewReportMetadataModel {
            event_id: metadata.event_id,
            app_version: metadata.app_version.clone(),
            platform: metadata.platform.clone(),
            environment: metadata.environment.clone(),
            error_type: metadata.error_type.clone(),
            error_message: metadata.error_message.clone(),
            sdk_name: metadata.sdk_name.clone(),
            sdk_version: metadata.sdk_version.clone(),
            processed_at: metadata.processed_at.naive_utc(),
        };

        diesel::insert_into(report_metadata::table)
            .values(&model)
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let id = diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>(
            "last_insert_rowid()",
        ))
        .get_result::<i32>(&mut conn)
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn find_by_event_id(&self, event_id: i32) -> Result<Option<ReportMetadata>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let result = report_metadata::table
            .filter(report_metadata::event_id.eq(event_id))
            .first::<ReportMetadataModel>(&mut conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| ReportMetadata {
            id: Some(m.id),
            event_id: m.event_id,
            app_version: m.app_version,
            platform: m.platform,
            environment: m.environment,
            error_type: m.error_type,
            error_message: m.error_message,
            sdk_name: m.sdk_name,
            sdk_version: m.sdk_version,
            processed_at: Utc.from_utc_datetime(&m.processed_at),
        }))
    }
}
