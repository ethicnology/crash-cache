ALTER TABLE report DROP CONSTRAINT report_project_event_id_key;
ALTER TABLE report ADD CONSTRAINT report_event_id_key UNIQUE(event_id);
