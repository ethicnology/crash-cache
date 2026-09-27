ALTER TABLE session
    ADD COLUMN distinct_id TEXT,
    ADD COLUMN sequence TEXT NOT NULL DEFAULT '0' CHECK (sequence ~ '^[0-9]+$' AND sequence::numeric <= 18446744073709551615),
    ADD COLUMN duration DOUBLE PRECISION CHECK (duration >= 0 AND duration < 'Infinity'::double precision),
    ADD COLUMN abnormal_mechanism TEXT;
-- Existing non-initial rows were stored without the logical clock. Reconstruct
-- their timestamp-based clock before accepting delayed updates after upgrade.
-- Refuse malformed legacy timestamps explicitly; never silently reset their state.
DO $$
DECLARE
    legacy_session RECORD;
    legacy_sequence TEXT;
BEGIN
    FOR legacy_session IN SELECT id, timestamp FROM session WHERE init <> 1 LOOP
        IF legacy_session.timestamp !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}[Tt][0-9]{2}:[0-9]{2}:[0-9]{2}([.][0-9]+)?([Zz]|[+-][0-9]{2}:[0-9]{2})$' THEN
            RAISE EXCEPTION 'Cannot migrate session %: legacy update timestamp is not RFC3339; repair explicitly before retrying', legacy_session.id;
        END IF;
        BEGIN
            legacy_sequence := floor(greatest(extract(epoch FROM regexp_replace(legacy_session.timestamp, '([.][0-9]{3})[0-9]+', '\1')::timestamptz) * 1000, 0))::text;
        EXCEPTION WHEN invalid_datetime_format OR datetime_field_overflow THEN
            RAISE EXCEPTION 'Cannot migrate session %: invalid legacy update timestamp; repair explicitly before retrying', legacy_session.id;
        END;
        UPDATE session SET sequence = legacy_sequence WHERE id = legacy_session.id;
    END LOOP;
END $$;

CREATE INDEX session_project_distinct_started_idx ON session(project_id, distinct_id, started_at);
ALTER TABLE report DROP CONSTRAINT report_event_id_key;
ALTER TABLE report ADD CONSTRAINT report_project_event_id_key UNIQUE(project_id, event_id);
