use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{DomainError, QueueItem, QueueError};
use crate::shared::persistence::sqlite::models::{NewQueueModel, QueueModel, NewQueueErrorModel, QueueErrorModel};
use crate::shared::persistence::sqlite::schema::{queue, queue_error};
use crate::shared::persistence::SqlitePool;

#[derive(Clone)]
pub struct QueueRepository {
    pool: SqlitePool,
}

impl QueueRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn enqueue(&self, item: &QueueItem) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let model = NewQueueModel {
            archive_hash: item.archive_hash.clone(),
            created_at: item.created_at.naive_utc(),
        };

        let rows = diesel::insert_or_ignore_into(queue::table)
            .values(&model)
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        if rows == 0 {
            let existing = queue::table
                .filter(queue::archive_hash.eq(&item.archive_hash))
                .select(queue::id)
                .first::<i32>(&mut conn)
                .map_err(|e| DomainError::Database(e.to_string()))?;
            return Ok(existing);
        }

        let id = diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>(
            "last_insert_rowid()",
        ))
        .get_result::<i32>(&mut conn)
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn dequeue_batch(&self, limit: i32) -> Result<Vec<QueueItem>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let results = queue::table
            .order(queue::created_at.asc())
            .limit(limit as i64)
            .load::<QueueModel>(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(|m| QueueItem {
                id: Some(m.id),
                archive_hash: m.archive_hash,
                created_at: Utc.from_utc_datetime(&m.created_at),
            })
            .collect())
    }

    pub fn remove(&self, archive_hash: &str) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        diesel::delete(queue::table.filter(queue::archive_hash.eq(archive_hash)))
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn count_pending(&self) -> Result<i64, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let count = queue::table
            .count()
            .get_result(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count)
    }
}

#[derive(Clone)]
pub struct QueueErrorRepository {
    pool: SqlitePool,
}

impl QueueErrorRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn record_error(&self, archive_hash: &str, error: &str) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let model = NewQueueErrorModel {
            archive_hash: archive_hash.to_string(),
            error: error.to_string(),
            created_at: Utc::now().naive_utc(),
        };

        // Use insert_or_ignore to handle duplicates gracefully
        let rows = diesel::insert_or_ignore_into(queue_error::table)
            .values(&model)
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        if rows == 0 {
            // Update existing error
            diesel::update(queue_error::table.filter(queue_error::archive_hash.eq(archive_hash)))
                .set((
                    queue_error::error.eq(error),
                    queue_error::created_at.eq(Utc::now().naive_utc()),
                ))
                .execute(&mut conn)
                .map_err(|e| DomainError::Database(e.to_string()))?;

            let existing = queue_error::table
                .filter(queue_error::archive_hash.eq(archive_hash))
                .select(queue_error::id)
                .first::<i32>(&mut conn)
                .map_err(|e| DomainError::Database(e.to_string()))?;
            return Ok(existing);
        }

        let id = diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>(
            "last_insert_rowid()",
        ))
        .get_result::<i32>(&mut conn)
        .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn find_all(&self) -> Result<Vec<QueueError>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let results = queue_error::table
            .order(queue_error::created_at.desc())
            .load::<QueueErrorModel>(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(|m| QueueError {
                id: m.id,
                archive_hash: m.archive_hash,
                error: m.error,
                created_at: Utc.from_utc_datetime(&m.created_at),
            })
            .collect())
    }

    pub fn remove(&self, archive_hash: &str) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        diesel::delete(queue_error::table.filter(queue_error::archive_hash.eq(archive_hash)))
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn count(&self) -> Result<i64, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let count = queue_error::table
            .count()
            .get_result(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count)
    }
}
