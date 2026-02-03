use super::DbPool;
use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{DomainError, Project};
use crate::shared::persistence::db::models::{NewProjectModel, ProjectModel};
use crate::shared::persistence::db::schema::project;

#[derive(Clone)]
pub struct ProjectRepository {
    pool: DbPool,
}

impl ProjectRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        public_key: Option<String>,
        name: Option<String>,
    ) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let model = NewProjectModel {
            public_key,
            name,
            created_at: chrono::Utc::now().naive_utc(),
        };

        diesel::insert_into(project::table)
            .values(&model)
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        #[cfg(feature = "sqlite")]
        let id: i32 = diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>(
            "last_insert_rowid()",
        ))
        .get_result(&mut conn)
        .map_err(|e| DomainError::Database(e.to_string()))?;

        #[cfg(feature = "postgres")]
        let id: i32 = project::table
            .select(project::id)
            .order(project::id.desc())
            .first(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn find_by_id(&self, id: i32) -> Result<Option<Project>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let result = project::table
            .filter(project::id.eq(id))
            .first::<ProjectModel>(&mut conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(result.map(|m| Project {
            id: m.id,
            public_key: m.public_key,
            name: m.name,
            created_at: Utc.from_utc_datetime(&m.created_at),
        }))
    }

    pub fn exists(&self, id: i32) -> Result<bool, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let count: i64 = project::table
            .filter(project::id.eq(id))
            .count()
            .get_result(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(count > 0)
    }

    /// Validates that the given public_key matches the project's stored key.
    /// Returns Ok(true) if valid, Ok(false) if invalid key, Err if project not found.
    pub fn validate_key(&self, id: i32, public_key: &str) -> Result<bool, DomainError> {
        let project = self.find_by_id(id)?;

        match project {
            Some(p) => match p.public_key {
                Some(stored_key) => Ok(stored_key == public_key),
                None => Ok(true), // No key configured = accept all
            },
            None => Err(DomainError::ProjectNotFound(id)),
        }
    }

    pub fn delete(&self, id: i32) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        diesel::delete(project::table.filter(project::id.eq(id)))
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn list_all(&self) -> Result<Vec<Project>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let results = project::table
            .order(project::created_at.desc())
            .load::<ProjectModel>(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(|m| Project {
                id: m.id,
                public_key: m.public_key,
                name: m.name,
                created_at: Utc.from_utc_datetime(&m.created_at),
            })
            .collect())
    }
}
