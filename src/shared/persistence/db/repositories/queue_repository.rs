use super::DbConnection;
use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{DomainError, QueueError, QueueItem};
use crate::shared::persistence::db::models::{
    NewQueueErrorModel, NewQueueModel, QueueErrorModel, QueueModel,
};
use crate::shared::persistence::db::schema::{queue, queue_error};

#[derive(Clone, Default)]
pub struct QueueRepository {}

impl QueueRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&self, conn: &mut DbConnection, item: &QueueItem) -> Result<i32, DomainError> {
        let model = NewQueueModel {
            archive_hash: item.archive_hash.clone(),
            created_at: item.created_at.naive_utc(),
        };

        // Try to insert and return the ID
        // If conflict occurs with do_nothing, Diesel will return an error, so we fetch existing
        match diesel::insert_into(queue::table)
            .values(&model)
            .returning(queue::id)
            .get_result::<i32>(conn)
        {
            Ok(id) => Ok(id),
            Err(_) => {
                // Conflict occurred, fetch the existing record
                let existing = queue::table
                    .filter(queue::archive_hash.eq(&item.archive_hash))
                    .select(queue::id)
                    .first::<i32>(conn)
                    .map_err(|e| DomainError::Database(e.to_string()))?;
                Ok(existing)
            }
        }
    }

    pub fn dequeue_batch(
        &self,
        conn: &mut DbConnection,
        limit: i32,
    ) -> Result<Vec<QueueItem>, DomainError> {
        let results = queue::table
            .order(queue::created_at.asc())
            .limit(limit as i64)
            .load::<QueueModel>(conn)
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

    pub fn remove(&self, conn: &mut DbConnection, archive_hash: &str) -> Result<(), DomainError> {
        diesel::delete(queue::table.filter(queue::archive_hash.eq(archive_hash)))
            .execute(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn count_pending(&self, conn: &mut DbConnection) -> Result<i64, DomainError> {
        let count = queue::table
            .count()
            .get_result(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count)
    }
}

#[derive(Clone, Default)]
pub struct QueueErrorRepository {}

impl QueueErrorRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_error(
        &self,
        conn: &mut DbConnection,
        archive_hash: &str,
        error: &str,
    ) -> Result<i32, DomainError> {
        let model = NewQueueErrorModel {
            archive_hash: archive_hash.to_string(),
            error: error.to_string(),
            created_at: Utc::now().naive_utc(),
        };

        // Try to insert and return ID
        match diesel::insert_into(queue_error::table)
            .values(&model)
            .returning(queue_error::id)
            .get_result::<i32>(conn)
        {
            Ok(id) => Ok(id),
            Err(_) => {
                // Conflict occurred, update existing record and return its ID
                let id = diesel::update(
                    queue_error::table.filter(queue_error::archive_hash.eq(archive_hash)),
                )
                .set((
                    queue_error::error.eq(error),
                    queue_error::created_at.eq(Utc::now().naive_utc()),
                ))
                .returning(queue_error::id)
                .get_result::<i32>(conn)
                .map_err(|e| DomainError::Database(e.to_string()))?;
                Ok(id)
            }
        }
    }

    pub fn find_all(&self, conn: &mut DbConnection) -> Result<Vec<QueueError>, DomainError> {
        let results = queue_error::table
            .order(queue_error::created_at.desc())
            .load::<QueueErrorModel>(conn)
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

    pub fn remove(&self, conn: &mut DbConnection, archive_hash: &str) -> Result<(), DomainError> {
        diesel::delete(queue_error::table.filter(queue_error::archive_hash.eq(archive_hash)))
            .execute(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn count(&self, conn: &mut DbConnection) -> Result<i64, DomainError> {
        let count = queue_error::table
            .count()
            .get_result(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count)
    }
}
