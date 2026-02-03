use super::DbPool;
use chrono::Utc;
use diesel::prelude::*;

use crate::shared::domain::DomainError;
use crate::shared::persistence::db::models::{NewReportModel, ReportModel};
use crate::shared::persistence::db::schema::report;

#[derive(Clone)]
pub struct ReportRepository {
    pool: DbPool,
}

#[derive(Debug, Clone, Default)]
pub struct NewReport {
    pub event_id: String,
    pub archive_hash: String,
    pub timestamp: i64,
    pub project_id: i32,
    pub platform_id: Option<i32>,
    pub environment_id: Option<i32>,
    pub os_name_id: Option<i32>,
    pub os_version_id: Option<i32>,
    pub manufacturer_id: Option<i32>,
    pub brand_id: Option<i32>,
    pub model_id: Option<i32>,
    pub chipset_id: Option<i32>,
    pub device_specs_id: Option<i32>,
    pub locale_code_id: Option<i32>,
    pub timezone_id: Option<i32>,
    pub connection_type_id: Option<i32>,
    pub orientation_id: Option<i32>,
    pub app_name_id: Option<i32>,
    pub app_version_id: Option<i32>,
    pub app_build_id: Option<i32>,
    pub user_id: Option<i32>,
    pub exception_type_id: Option<i32>,
    pub exception_message_id: Option<i32>,
    pub stacktrace_id: Option<i32>,
    pub issue_id: Option<i32>,
    pub session_id: Option<i32>,
}

impl ReportRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, new_report: NewReport) -> Result<i32, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        let exists: i64 = report::table
            .filter(report::event_id.eq(&new_report.event_id))
            .count()
            .get_result(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        if exists > 0 {
            return Err(DomainError::DuplicateEventId(new_report.event_id));
        }

        let model = NewReportModel {
            event_id: new_report.event_id,
            archive_hash: new_report.archive_hash,
            timestamp: new_report.timestamp,
            received_at: Utc::now().naive_utc(),
            project_id: new_report.project_id,
            platform_id: new_report.platform_id,
            environment_id: new_report.environment_id,
            os_name_id: new_report.os_name_id,
            os_version_id: new_report.os_version_id,
            manufacturer_id: new_report.manufacturer_id,
            brand_id: new_report.brand_id,
            model_id: new_report.model_id,
            chipset_id: new_report.chipset_id,
            device_specs_id: new_report.device_specs_id,
            locale_code_id: new_report.locale_code_id,
            timezone_id: new_report.timezone_id,
            connection_type_id: new_report.connection_type_id,
            orientation_id: new_report.orientation_id,
            app_name_id: new_report.app_name_id,
            app_version_id: new_report.app_version_id,
            app_build_id: new_report.app_build_id,
            user_id: new_report.user_id,
            exception_type_id: new_report.exception_type_id,
            exception_message_id: new_report.exception_message_id,
            stacktrace_id: new_report.stacktrace_id,
            issue_id: new_report.issue_id,
            session_id: new_report.session_id,
        };

        let id = diesel::insert_into(report::table)
            .values(&model)
            .returning(report::id)
            .get_result::<i32>(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(id)
    }

    pub fn find_by_event_id(&self, event_id: &str) -> Result<Option<ReportModel>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        report::table
            .filter(report::event_id.eq(event_id))
            .select(ReportModel::as_select())
            .first::<ReportModel>(&mut conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))
    }

    pub fn find_by_id(&self, id: i32) -> Result<Option<ReportModel>, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        report::table
            .filter(report::id.eq(id))
            .select(ReportModel::as_select())
            .first::<ReportModel>(&mut conn)
            .optional()
            .map_err(|e| DomainError::Database(e.to_string()))
    }

    pub fn count_by_project(&self, project_id: i32) -> Result<i64, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        report::table
            .filter(report::project_id.eq(project_id))
            .count()
            .get_result(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))
    }

    pub fn count_by_issue(&self, issue_id: i32) -> Result<i64, DomainError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        report::table
            .filter(report::issue_id.eq(issue_id))
            .count()
            .get_result(&mut conn)
            .map_err(|e| DomainError::Database(e.to_string()))
    }
}
