-- Drop indexes
DROP INDEX IF EXISTS idx_unwrap_stacktrace_fingerprint;
DROP INDEX IF EXISTS idx_report_user;
DROP INDEX IF EXISTS idx_report_issue;
DROP INDEX IF EXISTS idx_report_timestamp;
DROP INDEX IF EXISTS idx_report_project;
DROP INDEX IF EXISTS idx_archive_project;

-- Drop main tables (reverse order of dependencies)
DROP TABLE IF EXISTS report;
DROP TABLE IF EXISTS issue;

-- Drop unwrap tables
DROP TABLE IF EXISTS unwrap_stacktrace;
DROP TABLE IF EXISTS unwrap_exception_message;
DROP TABLE IF EXISTS unwrap_device_specs;
DROP TABLE IF EXISTS unwrap_exception_type;
DROP TABLE IF EXISTS unwrap_user;
DROP TABLE IF EXISTS unwrap_app_build;
DROP TABLE IF EXISTS unwrap_app_version;
DROP TABLE IF EXISTS unwrap_app_name;
DROP TABLE IF EXISTS unwrap_timezone;
DROP TABLE IF EXISTS unwrap_locale_code;
DROP TABLE IF EXISTS unwrap_chipset;
DROP TABLE IF EXISTS unwrap_model;
DROP TABLE IF EXISTS unwrap_brand;
DROP TABLE IF EXISTS unwrap_manufacturer;
DROP TABLE IF EXISTS unwrap_os_version;
DROP TABLE IF EXISTS unwrap_os_name;
DROP TABLE IF EXISTS unwrap_orientation;
DROP TABLE IF EXISTS unwrap_connection_type;
DROP TABLE IF EXISTS unwrap_environment;
DROP TABLE IF EXISTS unwrap_platform;

-- Drop core tables
DROP TABLE IF EXISTS queue_error;
DROP TABLE IF EXISTS queue;
DROP TABLE IF EXISTS archive;
DROP TABLE IF EXISTS project;
