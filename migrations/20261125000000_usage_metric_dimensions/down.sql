DELETE FROM metric_hour WHERE name IN ('bull.payment.completed', 'bull.swap.completed');
ALTER TABLE metric_hour
    DROP CONSTRAINT metric_hour_pkey,
    DROP COLUMN destination_network,
    DROP COLUMN direction,
    ADD PRIMARY KEY (project_id, bucket_start, name, release, environment, from_schema, to_schema,
                     chain, outcome);
