-- ============================================
-- CORE TABLES
-- ============================================

CREATE TABLE IF NOT EXISTS project (
    id SERIAL PRIMARY KEY,
    public_key TEXT UNIQUE,
    name TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS archive (
    hash TEXT PRIMARY KEY NOT NULL,
    project_id INTEGER NOT NULL,
    compressed_payload BYTEA NOT NULL,
    original_size INTEGER,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    FOREIGN KEY (project_id) REFERENCES project(id)
);

CREATE TABLE IF NOT EXISTS queue (
    id SERIAL PRIMARY KEY,
    archive_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    FOREIGN KEY (archive_hash) REFERENCES archive(hash)
);

CREATE TABLE IF NOT EXISTS queue_error (
    id SERIAL PRIMARY KEY,
    archive_hash TEXT NOT NULL UNIQUE,
    error TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    FOREIGN KEY (archive_hash) REFERENCES archive(hash)
);

-- ============================================
-- SESSION TABLES
-- ============================================

CREATE TABLE IF NOT EXISTS unwrap_session_status (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_session_release (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_session_environment (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS session (
    id SERIAL PRIMARY KEY,
    project_id INTEGER NOT NULL,
    sid TEXT NOT NULL,
    init INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    errors INTEGER NOT NULL DEFAULT 0,
    status_id INTEGER NOT NULL,
    release_id INTEGER,
    environment_id INTEGER,
    UNIQUE(project_id, sid),
    FOREIGN KEY (project_id) REFERENCES project(id),
    FOREIGN KEY (status_id) REFERENCES unwrap_session_status(id),
    FOREIGN KEY (release_id) REFERENCES unwrap_session_release(id),
    FOREIGN KEY (environment_id) REFERENCES unwrap_session_environment(id)
);

-- ============================================
-- UNWRAP TABLES (id / value UNIQUE)
-- ============================================

CREATE TABLE IF NOT EXISTS unwrap_platform (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_environment (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_connection_type (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_orientation (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_os_name (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_os_version (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_manufacturer (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_brand (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_model (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_chipset (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_locale_code (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_timezone (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_app_name (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_app_version (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_app_build (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_user (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_exception_type (
    id SERIAL PRIMARY KEY,
    value TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_device_specs (
    id SERIAL PRIMARY KEY,
    screen_width INTEGER,
    screen_height INTEGER,
    screen_density REAL,
    screen_dpi INTEGER,
    processor_count INTEGER,
    memory_size BIGINT,
    archs TEXT,
    UNIQUE(screen_width, screen_height, screen_density, screen_dpi, processor_count, memory_size, archs)
);

CREATE TABLE IF NOT EXISTS unwrap_exception_message (
    id SERIAL PRIMARY KEY,
    hash TEXT UNIQUE NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS unwrap_stacktrace (
    id SERIAL PRIMARY KEY,
    hash TEXT UNIQUE NOT NULL,
    fingerprint_hash TEXT,
    frames JSONB NOT NULL
);

-- ============================================
-- ISSUE TABLE
-- ============================================

CREATE TABLE IF NOT EXISTS issue (
    id SERIAL PRIMARY KEY,
    fingerprint_hash TEXT UNIQUE NOT NULL,
    exception_type_id INTEGER,
    title TEXT,
    first_seen TIMESTAMP NOT NULL,
    last_seen TIMESTAMP NOT NULL,
    event_count INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (exception_type_id) REFERENCES unwrap_exception_type(id)
);

-- ============================================
-- REPORT TABLE
-- ============================================

CREATE TABLE IF NOT EXISTS report (
    id SERIAL PRIMARY KEY,
    event_id TEXT UNIQUE NOT NULL,
    archive_hash TEXT NOT NULL,
    timestamp BIGINT NOT NULL,
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
    session_id INTEGER,

    FOREIGN KEY (archive_hash) REFERENCES archive(hash),
    FOREIGN KEY (project_id) REFERENCES project(id),
    FOREIGN KEY (platform_id) REFERENCES unwrap_platform(id),
    FOREIGN KEY (environment_id) REFERENCES unwrap_environment(id),
    FOREIGN KEY (os_name_id) REFERENCES unwrap_os_name(id),
    FOREIGN KEY (os_version_id) REFERENCES unwrap_os_version(id),
    FOREIGN KEY (manufacturer_id) REFERENCES unwrap_manufacturer(id),
    FOREIGN KEY (brand_id) REFERENCES unwrap_brand(id),
    FOREIGN KEY (model_id) REFERENCES unwrap_model(id),
    FOREIGN KEY (chipset_id) REFERENCES unwrap_chipset(id),
    FOREIGN KEY (device_specs_id) REFERENCES unwrap_device_specs(id),
    FOREIGN KEY (locale_code_id) REFERENCES unwrap_locale_code(id),
    FOREIGN KEY (timezone_id) REFERENCES unwrap_timezone(id),
    FOREIGN KEY (connection_type_id) REFERENCES unwrap_connection_type(id),
    FOREIGN KEY (orientation_id) REFERENCES unwrap_orientation(id),
    FOREIGN KEY (app_name_id) REFERENCES unwrap_app_name(id),
    FOREIGN KEY (app_version_id) REFERENCES unwrap_app_version(id),
    FOREIGN KEY (app_build_id) REFERENCES unwrap_app_build(id),
    FOREIGN KEY (user_id) REFERENCES unwrap_user(id),
    FOREIGN KEY (exception_type_id) REFERENCES unwrap_exception_type(id),
    FOREIGN KEY (exception_message_id) REFERENCES unwrap_exception_message(id),
    FOREIGN KEY (stacktrace_id) REFERENCES unwrap_stacktrace(id),
    FOREIGN KEY (issue_id) REFERENCES issue(id),
    FOREIGN KEY (session_id) REFERENCES session(id)
);

-- ============================================
-- INDEXES
-- ============================================

CREATE INDEX IF NOT EXISTS idx_archive_project ON archive(project_id);
CREATE INDEX IF NOT EXISTS idx_report_project ON report(project_id);
CREATE INDEX IF NOT EXISTS idx_report_timestamp ON report(timestamp);
CREATE INDEX IF NOT EXISTS idx_report_issue ON report(issue_id);
CREATE INDEX IF NOT EXISTS idx_report_user ON report(user_id);
CREATE INDEX IF NOT EXISTS idx_unwrap_stacktrace_fingerprint ON unwrap_stacktrace(fingerprint_hash);
CREATE INDEX IF NOT EXISTS idx_unwrap_stacktrace_frames ON unwrap_stacktrace USING GIN (frames);
CREATE INDEX IF NOT EXISTS idx_session_project ON session(project_id);
CREATE INDEX IF NOT EXISTS idx_session_status ON session(status_id);
CREATE INDEX IF NOT EXISTS idx_session_sid ON session(sid);
CREATE INDEX IF NOT EXISTS idx_report_session ON report(session_id);

-- ============================================
-- ANALYTICS BUCKET TABLES
-- ============================================

CREATE TABLE IF NOT EXISTS bucket_rate_limit_global (
    id SERIAL PRIMARY KEY,
    bucket_start TIMESTAMP NOT NULL UNIQUE,
    hit_count INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS bucket_rate_limit_dsn (
    id SERIAL PRIMARY KEY,
    dsn TEXT NOT NULL,
    project_id INTEGER,
    bucket_start TIMESTAMP NOT NULL,
    hit_count INTEGER NOT NULL DEFAULT 1,
    UNIQUE(dsn, bucket_start)
);

CREATE TABLE IF NOT EXISTS bucket_rate_limit_subnet (
    id SERIAL PRIMARY KEY,
    subnet TEXT NOT NULL,
    bucket_start TIMESTAMP NOT NULL,
    hit_count INTEGER NOT NULL DEFAULT 1,
    UNIQUE(subnet, bucket_start)
);

CREATE TABLE IF NOT EXISTS bucket_request_latency (
    id SERIAL PRIMARY KEY,
    endpoint TEXT NOT NULL,
    bucket_start TIMESTAMP NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 0,
    total_ms INTEGER NOT NULL DEFAULT 0,
    min_ms INTEGER,
    max_ms INTEGER,
    UNIQUE(endpoint, bucket_start)
);

CREATE INDEX IF NOT EXISTS idx_bucket_rate_limit_global_start ON bucket_rate_limit_global(bucket_start);
CREATE INDEX IF NOT EXISTS idx_bucket_rate_limit_dsn_start ON bucket_rate_limit_dsn(bucket_start);
CREATE INDEX IF NOT EXISTS idx_bucket_rate_limit_subnet_start ON bucket_rate_limit_subnet(bucket_start);
CREATE INDEX IF NOT EXISTS idx_bucket_request_latency_start ON bucket_request_latency(bucket_start);
