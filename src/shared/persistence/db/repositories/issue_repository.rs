use super::{DbConnection, DbPool};
use crate::shared::domain::DomainError;
use crate::shared::persistence::db::models::{IssueModel, NewIssueModel};
use crate::shared::persistence::db::schema::issue;
use chrono::Utc;
use diesel::prelude::*;

#[derive(Clone)]
pub struct IssueRepository {
    pool: DbPool,
}

impl IssueRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn get_or_create(
        &self,
        fingerprint_hash: &str,
        exception_type_id: Option<i32>,
        title: Option<String>,
    ) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.get_or_create_with_conn(&mut conn, fingerprint_hash, exception_type_id, title)
    }

    pub fn get_or_create_with_conn(
        &self,
        conn: &mut DbConnection,
        fingerprint_hash: &str,
        exception_type_id: Option<i32>,
        title: Option<String>,
    ) -> Result<i32, DomainError> {
        if let Some(existing) = issue::table
            .filter(issue::fingerprint_hash.eq(fingerprint_hash))
            .select(IssueModel::as_select())
            .first::<IssueModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?
        {
            let now = Utc::now().naive_utc();
            diesel::update(issue::table.filter(issue::id.eq(existing.id)))
                .set((
                    issue::last_seen.eq(now),
                    issue::event_count.eq(existing.event_count + 1),
                ))
                .execute(conn)
                .map_err(|e| DomainError::Database(e.to_string()))?;

            return Ok(existing.id);
        }

        let now = Utc::now().naive_utc();
        let new_record = NewIssueModel {
            fingerprint_hash: fingerprint_hash.to_string(),
            exception_type_id,
            title,
            first_seen: now,
            last_seen: now,
            event_count: 1,
        };

        let id = diesel::insert_into(issue::table)
            .values(&new_record)
            .returning(issue::id)
            .get_result::<i32>(conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn find_by_fingerprint(
        &self,
        fingerprint_hash: &str,
    ) -> Result<Option<IssueModel>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.find_by_fingerprint_with_conn(&mut conn, fingerprint_hash)
    }

    pub fn find_by_fingerprint_with_conn(
        &self,
        conn: &mut DbConnection,
        fingerprint_hash: &str,
    ) -> Result<Option<IssueModel>, DomainError> {
        issue::table
            .filter(issue::fingerprint_hash.eq(fingerprint_hash))
            .select(IssueModel::as_select())
            .first::<IssueModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))
    }

    pub fn find_by_id(&self, id: i32) -> Result<Option<IssueModel>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.find_by_id_with_conn(&mut conn, id)
    }

    pub fn find_by_id_with_conn(
        &self,
        conn: &mut DbConnection,
        id: i32,
    ) -> Result<Option<IssueModel>, DomainError> {
        issue::table
            .filter(issue::id.eq(id))
            .select(IssueModel::as_select())
            .first::<IssueModel>(conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))
    }

    pub fn list_all(&self) -> Result<Vec<IssueModel>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        self.list_all_with_conn(&mut conn)
    }

    pub fn list_all_with_conn(
        &self,
        conn: &mut DbConnection,
    ) -> Result<Vec<IssueModel>, DomainError> {
        issue::table
            .order(issue::last_seen.desc())
            .select(IssueModel::as_select())
            .load::<IssueModel>(conn)
            .map_err(|e| DomainError::Database(e.to_string()))
    }
}
