CREATE TABLE metric_replay (
    project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    digest TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    PRIMARY KEY (project_id, digest)
);
CREATE INDEX metric_replay_expiry_idx ON metric_replay(expires_at);

CREATE TABLE metric_hour (
    project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    bucket_start TIMESTAMP NOT NULL,
    name TEXT NOT NULL,
    release TEXT NOT NULL,
    environment TEXT NOT NULL,
    from_schema INTEGER NOT NULL,
    to_schema INTEGER NOT NULL,
    chain TEXT NOT NULL,
    outcome TEXT NOT NULL,
    sample_count BIGINT NOT NULL,
    value_sum DOUBLE PRECISION NOT NULL,
    value_min DOUBLE PRECISION NOT NULL,
    value_max DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (project_id, bucket_start, name, release, environment, from_schema, to_schema, chain, outcome)
);
CREATE INDEX metric_hour_expiry_idx ON metric_hour(bucket_start);
