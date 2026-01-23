-- ============================================
-- CORE TABLES
-- ============================================

CREATE TABLE IF NOT EXISTS project (
    id INTEGER PRIMARY KEY NOT NULL,
    public_key TEXT UNIQUE,
    name TEXT,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS archive (
    hash TEXT PRIMARY KEY NOT NULL,
    compressed_payload BLOB NOT NULL,
    original_size INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS processing_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    archive_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    next_retry_at TIMESTAMP,
    FOREIGN KEY (archive_hash) REFERENCES archive(hash)
);

-- ============================================
-- LOOKUP TABLES (id / value UNIQUE)
-- ============================================

CREATE TABLE IF NOT EXISTS lookup_platform (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_environment (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_connection_type (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_orientation (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_os_name (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_os_version (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_manufacturer (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_brand (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_model (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_chipset (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_locale_code (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_timezone (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_app_name (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_app_version (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_app_build (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_user (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS lookup_exception_type (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    value TEXT UNIQUE NOT NULL
);

-- ============================================
-- COMPOSITE TABLE
-- ============================================

CREATE TABLE IF NOT EXISTS device_specs (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    screen_width INTEGER,
    screen_height INTEGER,
    screen_density REAL,
    screen_dpi INTEGER,
    processor_count INTEGER,
    memory_size INTEGER,
    archs TEXT,
    UNIQUE(screen_width, screen_height, screen_density, screen_dpi, processor_count, memory_size, archs)
);

-- ============================================
-- EXCEPTION / STORAGE TABLES
-- ============================================

CREATE TABLE IF NOT EXISTS exception_message (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    hash TEXT UNIQUE NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS issue (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    fingerprint_hash TEXT UNIQUE NOT NULL,
    exception_type_id INTEGER,
    title TEXT,
    first_seen TIMESTAMP NOT NULL,
    last_seen TIMESTAMP NOT NULL,
    event_count INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (exception_type_id) REFERENCES lookup_exception_type(id)
);

CREATE TABLE IF NOT EXISTS stacktrace (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    hash TEXT UNIQUE NOT NULL,
    fingerprint_hash TEXT,
    frames_json BLOB NOT NULL,
    FOREIGN KEY (fingerprint_hash) REFERENCES issue(fingerprint_hash)
);

-- ============================================
-- REPORT TABLE
-- ============================================

CREATE TABLE IF NOT EXISTS report (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    event_id TEXT UNIQUE NOT NULL,
    archive_hash TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    received_at TIMESTAMP NOT NULL,

    project_id INTEGER NOT NULL,
    platform_id INTEGER,
    environment_id INTEGER,

    os_name_id INTEGER,
    os_version_id INTEGER,

    manufacturer_id INTEGER,
    brand_id INTEGER,
    model_id INTEGER,
    chipset_id INTEGER,
    device_specs_id INTEGER,

    locale_code_id INTEGER,
    timezone_id INTEGER,
    connection_type_id INTEGER,
    orientation_id INTEGER,

    app_name_id INTEGER,
    app_version_id INTEGER,
    app_build_id INTEGER,

    user_id INTEGER,

    exception_type_id INTEGER,
    exception_message_id INTEGER,
    stacktrace_id INTEGER,
    issue_id INTEGER,

    FOREIGN KEY (archive_hash) REFERENCES archive(hash),
    FOREIGN KEY (project_id) REFERENCES project(id),
    FOREIGN KEY (platform_id) REFERENCES lookup_platform(id),
    FOREIGN KEY (environment_id) REFERENCES lookup_environment(id),
    FOREIGN KEY (os_name_id) REFERENCES lookup_os_name(id),
    FOREIGN KEY (os_version_id) REFERENCES lookup_os_version(id),
    FOREIGN KEY (manufacturer_id) REFERENCES lookup_manufacturer(id),
    FOREIGN KEY (brand_id) REFERENCES lookup_brand(id),
    FOREIGN KEY (model_id) REFERENCES lookup_model(id),
    FOREIGN KEY (chipset_id) REFERENCES lookup_chipset(id),
    FOREIGN KEY (device_specs_id) REFERENCES device_specs(id),
    FOREIGN KEY (locale_code_id) REFERENCES lookup_locale_code(id),
    FOREIGN KEY (timezone_id) REFERENCES lookup_timezone(id),
    FOREIGN KEY (connection_type_id) REFERENCES lookup_connection_type(id),
    FOREIGN KEY (orientation_id) REFERENCES lookup_orientation(id),
    FOREIGN KEY (app_name_id) REFERENCES lookup_app_name(id),
    FOREIGN KEY (app_version_id) REFERENCES lookup_app_version(id),
    FOREIGN KEY (app_build_id) REFERENCES lookup_app_build(id),
    FOREIGN KEY (user_id) REFERENCES lookup_user(id),
    FOREIGN KEY (exception_type_id) REFERENCES lookup_exception_type(id),
    FOREIGN KEY (exception_message_id) REFERENCES exception_message(id),
    FOREIGN KEY (stacktrace_id) REFERENCES stacktrace(id),
    FOREIGN KEY (issue_id) REFERENCES issue(id)
);

-- ============================================
-- INDEXES
-- ============================================

CREATE INDEX IF NOT EXISTS idx_report_project ON report(project_id);
CREATE INDEX IF NOT EXISTS idx_report_timestamp ON report(timestamp);
CREATE INDEX IF NOT EXISTS idx_report_issue ON report(issue_id);
CREATE INDEX IF NOT EXISTS idx_report_user ON report(user_id);
CREATE INDEX IF NOT EXISTS idx_processing_queue_next_retry ON processing_queue(next_retry_at);
CREATE INDEX IF NOT EXISTS idx_stacktrace_fingerprint ON stacktrace(fingerprint_hash);
