use chrono::{TimeZone, Utc};
use diesel::prelude::*;

use crate::shared::domain::{DomainError, Project};
use crate::shared::persistence::sqlite::models::ProjectModel;
use crate::shared::persistence::sqlite::schema::project;
use crate::shared::persistence::SqlitePool;

#[derive(Clone)]
pub struct ProjectRepository {
    pool: SqlitePool,
}

impl ProjectRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn save(&self, proj: &Project) -> Result<(), DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let model = ProjectModel {
            id: proj.id.clone(),
            public_key: proj.public_key.clone(),
            name: proj.name.clone(),
            created_at: proj.created_at.naive_utc(),
        };

        diesel::insert_into(project::table)
            .values(&model)
            .execute(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<Project>, DomainError> {
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

    pub fn exists(&self, id: &str) -> Result<bool, DomainError> {
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

    pub fn delete(&self, id: &str) -> Result<(), DomainError> {
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
