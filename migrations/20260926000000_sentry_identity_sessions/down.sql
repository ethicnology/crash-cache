-- Refuse rollback if different projects contain the same event ID; never delete reports to downgrade.
ALTER TABLE report ADD CONSTRAINT report_event_id_key UNIQUE(event_id);
ALTER TABLE report DROP CONSTRAINT report_project_event_id_key;
DROP INDEX session_project_distinct_started_idx;
ALTER TABLE session DROP COLUMN distinct_id, DROP COLUMN sequence, DROP COLUMN duration, DROP COLUMN abnormal_mechanism;
