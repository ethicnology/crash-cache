use chrono::NaiveDateTime;
use diesel::prelude::*;

use super::schema::{archive, event, processing_queue, project, report_metadata};

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = project)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ProjectModel {
    pub id: i32,
    pub public_key: Option<String>,
    pub name: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = archive)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ArchiveModel {
    pub hash: String,
    pub compressed_payload: Vec<u8>,
    pub original_size: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = event)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct EventModel {
    pub id: i32,
    pub project_id: i32,
    pub archive_hash: String,
    pub received_at: NaiveDateTime,
    pub processed: bool,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = event)]
pub struct NewEventModel {
    pub project_id: i32,
    pub archive_hash: String,
    pub received_at: NaiveDateTime,
    pub processed: bool,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = processing_queue)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ProcessingQueueModel {
    pub id: i32,
    pub event_id: i32,
    pub created_at: NaiveDateTime,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<NaiveDateTime>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = processing_queue)]
pub struct NewProcessingQueueModel {
    pub event_id: i32,
    pub created_at: NaiveDateTime,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<NaiveDateTime>,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = report_metadata)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ReportMetadataModel {
    pub id: i32,
    pub event_id: i32,
    pub app_version: Option<String>,
    pub platform: Option<String>,
    pub environment: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub sdk_name: Option<String>,
    pub sdk_version: Option<String>,
    pub processed_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = report_metadata)]
pub struct NewReportMetadataModel {
    pub event_id: i32,
    pub app_version: Option<String>,
    pub platform: Option<String>,
    pub environment: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub sdk_name: Option<String>,
    pub sdk_version: Option<String>,
    pub processed_at: NaiveDateTime,
}
