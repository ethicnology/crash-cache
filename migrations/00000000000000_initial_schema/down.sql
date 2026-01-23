-- Drop indexes
DROP INDEX IF EXISTS idx_stacktrace_fingerprint;
DROP INDEX IF EXISTS idx_processing_queue_next_retry;
DROP INDEX IF EXISTS idx_report_user;
DROP INDEX IF EXISTS idx_report_issue;
DROP INDEX IF EXISTS idx_report_timestamp;
DROP INDEX IF EXISTS idx_report_project;

-- Drop main tables (reverse order of dependencies)
DROP TABLE IF EXISTS report;
DROP TABLE IF EXISTS stacktrace;
DROP TABLE IF EXISTS issue;
DROP TABLE IF EXISTS exception_message;
DROP TABLE IF EXISTS device_specs;

-- Drop lookup tables
DROP TABLE IF EXISTS lookup_exception_type;
DROP TABLE IF EXISTS lookup_user;
DROP TABLE IF EXISTS lookup_app_build;
DROP TABLE IF EXISTS lookup_app_version;
DROP TABLE IF EXISTS lookup_app_name;
DROP TABLE IF EXISTS lookup_timezone;
DROP TABLE IF EXISTS lookup_locale_code;
DROP TABLE IF EXISTS lookup_chipset;
DROP TABLE IF EXISTS lookup_model;
DROP TABLE IF EXISTS lookup_brand;
DROP TABLE IF EXISTS lookup_manufacturer;
DROP TABLE IF EXISTS lookup_os_version;
DROP TABLE IF EXISTS lookup_os_name;
DROP TABLE IF EXISTS lookup_orientation;
DROP TABLE IF EXISTS lookup_connection_type;
DROP TABLE IF EXISTS lookup_environment;
DROP TABLE IF EXISTS lookup_platform;

-- Drop core tables
DROP TABLE IF EXISTS processing_queue;
DROP TABLE IF EXISTS archive;
DROP TABLE IF EXISTS project;
