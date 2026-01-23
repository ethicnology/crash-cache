use chrono::NaiveDateTime;
use diesel::prelude::*;

use super::schema::*;

// ============================================
// CORE MODELS
// ============================================

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = project)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ProjectModel {
    pub id: i32,
    pub public_key: Option<String>,
    pub name: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = project)]
pub struct NewProjectModel {
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
    pub original_size: Option<i32>,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = processing_queue)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ProcessingQueueModel {
    pub id: i32,
    pub archive_hash: String,
    pub created_at: NaiveDateTime,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<NaiveDateTime>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = processing_queue)]
pub struct NewProcessingQueueModel {
    pub archive_hash: String,
    pub created_at: NaiveDateTime,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<NaiveDateTime>,
}

// ============================================
// LOOKUP MODELS (generic pattern)
// ============================================

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_platform)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupPlatformModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_platform)]
pub struct NewLookupPlatformModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_environment)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupEnvironmentModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_environment)]
pub struct NewLookupEnvironmentModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_connection_type)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupConnectionTypeModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_connection_type)]
pub struct NewLookupConnectionTypeModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_orientation)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupOrientationModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_orientation)]
pub struct NewLookupOrientationModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_os_name)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupOsNameModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_os_name)]
pub struct NewLookupOsNameModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_os_version)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupOsVersionModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_os_version)]
pub struct NewLookupOsVersionModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_manufacturer)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupManufacturerModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_manufacturer)]
pub struct NewLookupManufacturerModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_brand)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupBrandModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_brand)]
pub struct NewLookupBrandModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_model)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupModelModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_model)]
pub struct NewLookupModelModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_chipset)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupChipsetModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_chipset)]
pub struct NewLookupChipsetModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_locale_code)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupLocaleCodeModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_locale_code)]
pub struct NewLookupLocaleCodeModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_timezone)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupTimezoneModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_timezone)]
pub struct NewLookupTimezoneModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_app_name)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupAppNameModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_app_name)]
pub struct NewLookupAppNameModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_app_version)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupAppVersionModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_app_version)]
pub struct NewLookupAppVersionModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_app_build)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupAppBuildModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_app_build)]
pub struct NewLookupAppBuildModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_user)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupUserModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_user)]
pub struct NewLookupUserModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = lookup_exception_type)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LookupExceptionTypeModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = lookup_exception_type)]
pub struct NewLookupExceptionTypeModel {
    pub value: String,
}

// ============================================
// COMPOSITE MODEL
// ============================================

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = device_specs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DeviceSpecsModel {
    pub id: i32,
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub screen_density: Option<f32>,
    pub screen_dpi: Option<i32>,
    pub processor_count: Option<i32>,
    pub memory_size: Option<i64>,
    pub archs: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_specs)]
pub struct NewDeviceSpecsModel {
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub screen_density: Option<f32>,
    pub screen_dpi: Option<i32>,
    pub processor_count: Option<i32>,
    pub memory_size: Option<i64>,
    pub archs: Option<String>,
}

// ============================================
// EXCEPTION / STORAGE MODELS
// ============================================

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = exception_message)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ExceptionMessageModel {
    pub id: i32,
    pub hash: String,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = exception_message)]
pub struct NewExceptionMessageModel {
    pub hash: String,
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = issue)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct IssueModel {
    pub id: i32,
    pub fingerprint_hash: String,
    pub exception_type_id: Option<i32>,
    pub title: Option<String>,
    pub first_seen: NaiveDateTime,
    pub last_seen: NaiveDateTime,
    pub event_count: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = issue)]
pub struct NewIssueModel {
    pub fingerprint_hash: String,
    pub exception_type_id: Option<i32>,
    pub title: Option<String>,
    pub first_seen: NaiveDateTime,
    pub last_seen: NaiveDateTime,
    pub event_count: i32,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = stacktrace)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct StacktraceModel {
    pub id: i32,
    pub hash: String,
    pub fingerprint_hash: Option<String>,
    pub frames_json: Vec<u8>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = stacktrace)]
pub struct NewStacktraceModel {
    pub hash: String,
    pub fingerprint_hash: Option<String>,
    pub frames_json: Vec<u8>,
}

// ============================================
// REPORT MODEL
// ============================================

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = report)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ReportModel {
    pub id: i32,
    pub event_id: String,
    pub archive_hash: String,
    pub timestamp: i64,
    pub received_at: NaiveDateTime,

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
}

#[derive(Insertable, Debug)]
#[diesel(table_name = report)]
pub struct NewReportModel {
    pub event_id: String,
    pub archive_hash: String,
    pub timestamp: i64,
    pub received_at: NaiveDateTime,

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
}
