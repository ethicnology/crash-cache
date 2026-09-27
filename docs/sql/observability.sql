-- Run against a migrated database with a read-only account:
-- psql "$DATABASE_URL" -v project_id=1 -v from=2026-09-01T00:00:00Z \
--   -v until=2026-10-01T00:00:00Z -f docs/sql/observability.sql
-- Bounds are inclusive/exclusive. Calendar weeks start on Monday in UTC.
BEGIN READ ONLY;
SET LOCAL TIME ZONE 'UTC';

-- Sessions and installation counts by session-start period, not people or concurrent players.
WITH starts AS (
    SELECT sid, distinct_id, started_at::timestamptz AS started
    FROM session
    WHERE project_id = :project_id
      AND started_at::timestamptz >= :'from'::timestamptz
      AND started_at::timestamptz < :'until'::timestamptz
)
SELECT period.grain, date_trunc(period.grain, starts.started AT TIME ZONE 'UTC') AS period_start,
       count(*) AS sessions_started,
       count(DISTINCT distinct_id) AS installations_with_session_starts,
       count(*) FILTER (WHERE distinct_id IS NULL) AS sessions_without_installation
FROM starts CROSS JOIN (VALUES ('day'), ('week'), ('month')) AS period(grain)
GROUP BY period.grain, period_start
ORDER BY period.grain, period_start;

-- Recorded foreground activity, including sessions crossing UTC date boundaries.
-- Start/resume/foreground heartbeats use info events; never infer activity from crashes.
WITH activity AS (
    SELECT r.timestamp, u.value AS installation
    FROM report r
    JOIN unwrap_user u ON u.id = r.user_id
    WHERE r.project_id = :project_id
      AND to_timestamp(r.timestamp) >= :'from'::timestamptz
      AND to_timestamp(r.timestamp) < :'until'::timestamptz
      AND EXISTS (
          SELECT 1 FROM report_tag t
          JOIN unwrap_tag_key tk ON tk.id = t.key_id
          JOIN unwrap_tag_value tv ON tv.id = t.value_id
          WHERE t.report_id = r.id AND tk.value = 'event_kind'
            AND tv.value IN ('app_session', 'app_activity')
      )
)
SELECT period.grain, date_trunc(period.grain, to_timestamp(activity.timestamp) AT TIME ZONE 'UTC') AS period_start,
       count(DISTINCT activity.installation) AS observed_active_installations,
       count(*) AS activity_observations
FROM activity CROSS JOIN (VALUES ('day'), ('week'), ('month')) AS period(grain)
GROUP BY period.grain, period_start
ORDER BY period.grain, period_start;

-- Release health: incomplete/abnormal sessions remain visible instead of being called healthy.
SELECT release.value AS release, environment.value AS environment,
       count(*) AS sessions,
       count(*) FILTER (WHERE status.value = 'crashed') AS crashes,
       count(*) FILTER (WHERE status.value = 'abnormal') AS abnormal,
       count(*) FILTER (WHERE status.value = 'unhandled') AS unhandled,
       count(*) FILTER (WHERE status.value = 'ok') AS still_open,
       count(*) FILTER (WHERE s.errors > 0) AS sessions_with_errors,
       round(100.0 * count(*) FILTER (WHERE status.value <> 'crashed') /
             NULLIF(count(*), 0), 2) AS percent_without_reported_crash,
       avg(s.duration) FILTER (WHERE status.value = 'exited') AS mean_exited_duration_seconds
FROM session s
JOIN unwrap_session_status status ON status.id = s.status_id
LEFT JOIN unwrap_session_release release ON release.id = s.release_id
LEFT JOIN unwrap_session_environment environment ON environment.id = s.environment_id
WHERE s.project_id = :project_id
  AND s.started_at::timestamptz >= :'from'::timestamptz
  AND s.started_at::timestamptz < :'until'::timestamptz
GROUP BY release.value, environment.value
ORDER BY release.value, environment.value;

-- Healthy observations are standard info events with explicit opt-in tags, not fake exceptions.
WITH observation AS (
    SELECT r.*,
           (SELECT tv.value FROM report_tag t
            JOIN unwrap_tag_key tk ON tk.id = t.key_id
            JOIN unwrap_tag_value tv ON tv.id = t.value_id
            WHERE t.report_id = r.id AND tk.value = 'app_session_id') AS app_session_id
    FROM report r
    WHERE r.project_id = :project_id
      AND to_timestamp(r.timestamp) >= :'from'::timestamptz
      AND to_timestamp(r.timestamp) < :'until'::timestamptz
      AND EXISTS (
          SELECT 1 FROM report_tag t
          JOIN unwrap_tag_key tk ON tk.id = t.key_id
          JOIN unwrap_tag_value tv ON tv.id = t.value_id
          WHERE t.report_id = r.id AND tk.value = 'event_kind' AND tv.value IN ('app_session', 'app_activity')
      )
)
SELECT platform.value AS runtime_platform, os.value AS os, version.value AS os_version, model.value AS device_model,
       app.value AS app_version, build.value AS app_build,
       count(DISTINCT u.value) AS observed_installations,
       count(DISTINCT s.id) AS correlated_sessions,
       count(*) FILTER (WHERE s.id IS NULL) AS uncorrelated_observations,
       count(*) AS observation_events
FROM observation r
LEFT JOIN unwrap_user u ON u.id = r.user_id
LEFT JOIN session s ON s.project_id = r.project_id
    AND r.app_session_id ~* '^([0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$'
    AND replace(lower(s.sid), '-', '') = replace(lower(r.app_session_id), '-', '')
    AND s.distinct_id = u.value
LEFT JOIN unwrap_platform platform ON platform.id = r.platform_id
LEFT JOIN unwrap_os_name os ON os.id = r.os_name_id
LEFT JOIN unwrap_os_version version ON version.id = r.os_version_id
LEFT JOIN unwrap_model model ON model.id = r.model_id
LEFT JOIN unwrap_app_version app ON app.id = r.app_version_id
LEFT JOIN unwrap_app_build build ON build.id = r.app_build_id
GROUP BY platform.value, os.value, version.value, model.value, app.value, build.value
ORDER BY observed_installations DESC;

-- Error occurrences by the runtime and feature that captured them.
SELECT coalesce(nullif(layer.value, ''), platform.value, 'Unclassified') AS layer, component.value AS component,
       count(*) AS error_events, count(DISTINCT r.issue_id) AS issues,
       count(DISTINCT r.user_id) AS affected_installations
FROM report r
LEFT JOIN unwrap_platform platform ON platform.id = r.platform_id
LEFT JOIN LATERAL (
    SELECT tv.value FROM report_tag t
    JOIN unwrap_tag_key tk ON tk.id = t.key_id
    JOIN unwrap_tag_value tv ON tv.id = t.value_id
    WHERE t.report_id = r.id AND tk.value = 'layer'
) layer ON true
LEFT JOIN LATERAL (
    SELECT tv.value FROM report_tag t
    JOIN unwrap_tag_key tk ON tk.id = t.key_id
    JOIN unwrap_tag_value tv ON tv.id = t.value_id
    WHERE t.report_id = r.id AND tk.value = 'component'
) component ON true
WHERE r.project_id = :project_id AND r.issue_id IS NOT NULL
  AND to_timestamp(r.timestamp) >= :'from'::timestamptz
  AND to_timestamp(r.timestamp) < :'until'::timestamptz
GROUP BY coalesce(nullif(layer.value, ''), platform.value, 'Unclassified'), component.value
ORDER BY error_events DESC;

-- A propagated failure has one occurrence ID and one capture owner. Multiple event IDs
-- for that occurrence indicate duplicate cross-layer capture, not a transport retry.
SELECT tv.value AS error_occurrence_id, count(*) AS captured_events,
       array_agg(r.event_id ORDER BY r.event_id) AS event_ids
FROM report r
JOIN report_tag t ON t.report_id = r.id
JOIN unwrap_tag_key tk ON tk.id = t.key_id
JOIN unwrap_tag_value tv ON tv.id = t.value_id
WHERE r.project_id = :project_id AND tk.value = 'error_occurrence_id'
  AND to_timestamp(r.timestamp) >= :'from'::timestamptz
  AND to_timestamp(r.timestamp) < :'until'::timestamptz
GROUP BY tv.value HAVING count(*) > 1
ORDER BY captured_events DESC;
COMMIT;
