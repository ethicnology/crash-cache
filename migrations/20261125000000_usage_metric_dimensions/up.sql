ALTER TABLE metric_hour
    ADD COLUMN destination_network TEXT NOT NULL DEFAULT '',
    ADD COLUMN direction TEXT NOT NULL DEFAULT '',
    DROP CONSTRAINT metric_hour_pkey,
    ADD PRIMARY KEY (project_id, bucket_start, name, release, environment, from_schema, to_schema,
                     chain, outcome, destination_network, direction);
