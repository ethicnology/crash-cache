use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{DomainError, ProcessingQueueItem};
use crate::shared::persistence::sqlite::models::{NewProcessingQueueModel, ProcessingQueueModel};
use crate::shared::persistence::sqlite::schema::processing_queue;
use crate::shared::persistence::SqlitePool;

#[derive(Clone)]
pub struct QueueRepository {
    pool: SqlitePool,
}

impl QueueRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn enqueue(&self, item: &ProcessingQueueItem) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let model = NewProcessingQueueModel {
            event_id: item.event_id,
            created_at: item.created_at.naive_utc(),
            retry_count: item.retry_count,
            last_error: item.last_error.clone(),
            next_retry_at: item.next_retry_at.map(|dt| dt.naive_utc()),
        };

        diesel::insert_into(processing_queue::table)
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

    pub fn dequeue_batch(&self, limit: i32) -> Result<Vec<ProcessingQueueItem>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let now = Utc::now().naive_utc();

        let results = processing_queue::table
            .filter(
                processing_queue::next_retry_at
                    .is_null()
                    .or(processing_queue::next_retry_at.le(now)),
            )
            .order(processing_queue::created_at.asc())
            .limit(limit as i64)
            .load::<ProcessingQueueModel>(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(|m| ProcessingQueueItem {
                id: Some(m.id),
                event_id: m.event_id,
                created_at: Utc.from_utc_datetime(&m.created_at),
                retry_count: m.retry_count,
                last_error: m.last_error,
                next_retry_at: m.next_retry_at.map(|dt| Utc.from_utc_datetime(&dt)),
            })
            .collect())
    }

    pub fn remove(&self, event_id: i32) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        diesel::delete(processing_queue::table.filter(processing_queue::event_id.eq(event_id)))
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn update_retry(&self, item: &ProcessingQueueItem) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        diesel::update(processing_queue::table.filter(processing_queue::event_id.eq(item.event_id)))
            .set((
                processing_queue::retry_count.eq(item.retry_count),
                processing_queue::last_error.eq(&item.last_error),
                processing_queue::next_retry_at.eq(item.next_retry_at.map(|dt| dt.naive_utc())),
            ))
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn count_pending(&self) -> Result<i64, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let count = processing_queue::table
            .count()
            .get_result(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count)
    }
}
