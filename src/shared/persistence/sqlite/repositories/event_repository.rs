use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{DomainError, Event};
use crate::shared::persistence::sqlite::models::{EventModel, NewEventModel};
use crate::shared::persistence::sqlite::schema::event;
use crate::shared::persistence::SqlitePool;

#[derive(Clone)]
pub struct EventRepository {
    pool: SqlitePool,
}

impl EventRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn save(&self, evt: &Event) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let model = NewEventModel {
            project_id: evt.project_id,
            archive_hash: evt.archive_hash.clone(),
            received_at: evt.received_at.naive_utc(),
            processed: evt.processed,
        };

        diesel::insert_into(event::table)
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

    pub fn find_by_id(&self, id: i32) -> Result<Option<Event>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let result = event::table
            .filter(event::id.eq(id))
            .first::<EventModel>(&mut conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| Event {
            id: Some(m.id),
            project_id: m.project_id,
            archive_hash: m.archive_hash,
            received_at: Utc.from_utc_datetime(&m.received_at),
            processed: m.processed,
        }))
    }

    pub fn mark_processed(&self, id: i32) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        diesel::update(event::table.filter(event::id.eq(id)))
            .set(event::processed.eq(true))
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }
}
