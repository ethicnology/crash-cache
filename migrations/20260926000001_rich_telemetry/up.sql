CREATE TABLE telemetry_log (
    id BIGSERIAL PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES project(id),
    archive_hash TEXT NOT NULL REFERENCES archive(hash),
    item_index INTEGER NOT NULL,
    log_index INTEGER NOT NULL,
    timestamp DOUBLE PRECISION NOT NULL CHECK (timestamp >= 0 AND timestamp < 253402300800),
    trace_id TEXT,
    span_id TEXT,
    level TEXT NOT NULL,
    body TEXT NOT NULL,
    severity_number INTEGER,
    attributes JSONB NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (archive_hash, item_index, log_index)
);
CREATE INDEX telemetry_log_project_time ON telemetry_log(project_id, timestamp);
CREATE INDEX telemetry_log_project_trace ON telemetry_log(project_id, trace_id);
CREATE TABLE attachment_metadata (
    id BIGSERIAL PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES project(id),
    archive_hash TEXT NOT NULL REFERENCES archive(hash),
    event_id TEXT,
    item_index INTEGER NOT NULL,
    filename TEXT,
    attachment_type TEXT,
    content_type TEXT,
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (archive_hash, item_index)
);
CREATE INDEX attachment_metadata_project_event ON attachment_metadata(project_id, event_id);
