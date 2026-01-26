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
    pub project_id: i32,
    pub compressed_payload: Vec<u8>,
    pub original_size: Option<i32>,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = queue)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct QueueModel {
    pub id: i32,
    pub archive_hash: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = queue)]
pub struct NewQueueModel {
    pub archive_hash: String,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = queue_error)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct QueueErrorModel {
    pub id: i32,
    pub archive_hash: String,
    pub error: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = queue_error)]
pub struct NewQueueErrorModel {
    pub archive_hash: String,
    pub error: String,
    pub created_at: NaiveDateTime,
}

// ============================================
// UNWRAP MODELS (generic pattern)
// ============================================

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_platform)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapPlatformModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_platform)]
pub struct NewUnwrapPlatformModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_environment)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapEnvironmentModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_environment)]
pub struct NewUnwrapEnvironmentModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_connection_type)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapConnectionTypeModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_connection_type)]
pub struct NewUnwrapConnectionTypeModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_orientation)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapOrientationModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_orientation)]
pub struct NewUnwrapOrientationModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_os_name)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapOsNameModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_os_name)]
pub struct NewUnwrapOsNameModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_os_version)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapOsVersionModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_os_version)]
pub struct NewUnwrapOsVersionModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_manufacturer)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapManufacturerModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_manufacturer)]
pub struct NewUnwrapManufacturerModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_brand)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapBrandModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_brand)]
pub struct NewUnwrapBrandModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_model)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapModelModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_model)]
pub struct NewUnwrapModelModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_chipset)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapChipsetModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_chipset)]
pub struct NewUnwrapChipsetModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_locale_code)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapLocaleCodeModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_locale_code)]
pub struct NewUnwrapLocaleCodeModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_timezone)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapTimezoneModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_timezone)]
pub struct NewUnwrapTimezoneModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_app_name)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapAppNameModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_app_name)]
pub struct NewUnwrapAppNameModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_app_version)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapAppVersionModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_app_version)]
pub struct NewUnwrapAppVersionModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_app_build)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapAppBuildModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_app_build)]
pub struct NewUnwrapAppBuildModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_user)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapUserModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_user)]
pub struct NewUnwrapUserModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_exception_type)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapExceptionTypeModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_exception_type)]
pub struct NewUnwrapExceptionTypeModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_device_specs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapDeviceSpecsModel {
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
#[diesel(table_name = unwrap_device_specs)]
pub struct NewUnwrapDeviceSpecsModel {
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub screen_density: Option<f32>,
    pub screen_dpi: Option<i32>,
    pub processor_count: Option<i32>,
    pub memory_size: Option<i64>,
    pub archs: Option<String>,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_exception_message)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapExceptionMessageModel {
    pub id: i32,
    pub hash: String,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_exception_message)]
pub struct NewUnwrapExceptionMessageModel {
    pub hash: String,
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_stacktrace)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnwrapStacktraceModel {
    pub id: i32,
    pub hash: String,
    pub fingerprint_hash: Option<String>,
    pub frames_json: Vec<u8>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_stacktrace)]
pub struct NewUnwrapStacktraceModel {
    pub hash: String,
    pub fingerprint_hash: Option<String>,
    pub frames_json: Vec<u8>,
}

// ============================================
// ISSUE MODEL
// ============================================

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
