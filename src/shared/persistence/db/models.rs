use chrono::NaiveDateTime;
use diesel::prelude::*;

use super::schema::{
    archive, bucket_rate_limit_dsn, bucket_rate_limit_global, bucket_rate_limit_subnet,
    bucket_request_latency, issue, project, queue, queue_error, report, session, unwrap_app_build,
    unwrap_app_name, unwrap_app_version, unwrap_brand, unwrap_chipset, unwrap_connection_type,
    unwrap_device_specs, unwrap_environment, unwrap_exception_message, unwrap_exception_type,
    unwrap_locale_code, unwrap_manufacturer, unwrap_model, unwrap_orientation, unwrap_os_name,
    unwrap_os_version, unwrap_platform, unwrap_session_environment, unwrap_session_release,
    unwrap_session_status, unwrap_stacktrace, unwrap_timezone, unwrap_user,
};

// ============================================
// CORE MODELS
// ============================================

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = project)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct ArchiveModel {
    pub hash: String,
    pub project_id: i32,
    pub compressed_payload: Vec<u8>,
    pub original_size: Option<i32>,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = queue)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
// SESSION MODELS
// ============================================

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_session_status)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct UnwrapSessionStatusModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_session_status)]
pub struct NewUnwrapSessionStatusModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_session_release)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct UnwrapSessionReleaseModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_session_release)]
pub struct NewUnwrapSessionReleaseModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_session_environment)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct UnwrapSessionEnvironmentModel {
    pub id: i32,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_session_environment)]
pub struct NewUnwrapSessionEnvironmentModel {
    pub value: String,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = session)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct SessionModel {
    pub id: i32,
    pub project_id: i32,
    pub sid: String,
    pub init: i32,
    pub started_at: String,
    pub timestamp: String,
    pub errors: i32,
    pub status_id: i32,
    pub release_id: Option<i32>,
    pub environment_id: Option<i32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = session)]
pub struct NewSessionModel {
    pub project_id: i32,
    pub sid: String,
    pub init: i32,
    pub started_at: String,
    pub timestamp: String,
    pub errors: i32,
    pub status_id: i32,
    pub release_id: Option<i32>,
    pub environment_id: Option<i32>,
}

// ============================================
// UNWRAP MODELS (generic pattern)
// ============================================

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = unwrap_platform)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct UnwrapStacktraceModel {
    pub id: i32,
    pub hash: String,
    pub fingerprint_hash: Option<String>,
    pub frames_json: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = unwrap_stacktrace)]
pub struct NewUnwrapStacktraceModel {
    pub hash: String,
    pub fingerprint_hash: Option<String>,
    pub frames_json: String,
}

// ============================================
// ISSUE MODEL
// ============================================

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = issue)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
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
    pub session_id: Option<i32>,
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
    pub session_id: Option<i32>,
}

// ============================================
// ANALYTICS BUCKET MODELS
// ============================================

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = bucket_rate_limit_global)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct BucketRateLimitGlobalModel {
    pub id: i32,
    pub bucket_start: NaiveDateTime,
    pub hit_count: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = bucket_rate_limit_global)]
pub struct NewBucketRateLimitGlobalModel {
    pub bucket_start: NaiveDateTime,
    pub hit_count: i32,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = bucket_rate_limit_dsn)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct BucketRateLimitDsnModel {
    pub id: i32,
    pub dsn: String,
    pub project_id: Option<i32>,
    pub bucket_start: NaiveDateTime,
    pub hit_count: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = bucket_rate_limit_dsn)]
pub struct NewBucketRateLimitDsnModel {
    pub dsn: String,
    pub project_id: Option<i32>,
    pub bucket_start: NaiveDateTime,
    pub hit_count: i32,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = bucket_rate_limit_subnet)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct BucketRateLimitSubnetModel {
    pub id: i32,
    pub subnet: String,
    pub bucket_start: NaiveDateTime,
    pub hit_count: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = bucket_rate_limit_subnet)]
pub struct NewBucketRateLimitSubnetModel {
    pub subnet: String,
    pub bucket_start: NaiveDateTime,
    pub hit_count: i32,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = bucket_request_latency)]
#[cfg_attr(feature = "sqlite", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
#[cfg_attr(feature = "postgres", diesel(check_for_backend(diesel::pg::Pg)))]
pub struct BucketRequestLatencyModel {
    pub id: i32,
    pub endpoint: String,
    pub bucket_start: NaiveDateTime,
    pub request_count: i32,
    pub total_ms: i32,
    pub min_ms: Option<i32>,
    pub max_ms: Option<i32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = bucket_request_latency)]
pub struct NewBucketRequestLatencyModel {
    pub endpoint: String,
    pub bucket_start: NaiveDateTime,
    pub request_count: i32,
    pub total_ms: i32,
    pub min_ms: Option<i32>,
    pub max_ms: Option<i32>,
}
