-- Authenticated investigation models for Metabase. Apply as the schema owner after
-- migrations through 20260926000001_rich_telemetry, with ON_ERROR_STOP enabled.
-- These ordinary views write no telemetry and require no archive decoding.
-- Grant USAGE on crash_cache_explorer and SELECT on its views to the Metabase role.
-- Do not grant its users access to public.project.public_key or archive payloads.
-- These views are NOT a tenant security boundary: project filters support analysis;
-- access restrictions must be enforced by database roles or Metabase permissions.
-- Full messages, variables and custom contexts are intended for authenticated staff.
-- Stable keys pair positive SERIAL project/report IDs with their integer child IDs.
-- Counters are lifetime values. For a time/environment/version slice, aggregate
-- reports AFTER filtering; filtering an issue's last_seen does not slice its count.
-- Breadcrumb timestamps are milliseconds; report and structured log times are seconds.
-- No explicit ordering is guaranteed by a view: order breadcrumbs by chronology,
-- frames by position, and reports by event_at plus id in the consuming question.
BEGIN;
CREATE SCHEMA IF NOT EXISTS crash_cache_explorer;

-- A bounded date dimension supports relative/custom ranges even with no telemetry.
CREATE OR REPLACE VIEW crash_cache_explorer.calendar AS
SELECT date '2000-01-01' + n AS day
FROM generate_series(0, date '2100-12-31' - date '2000-01-01') AS n;

CREATE OR REPLACE VIEW crash_cache_explorer.projects AS
SELECT id, name, created_at FROM public.project;

CREATE OR REPLACE VIEW crash_cache_explorer.reports AS
SELECT r.id, r.project_id,
       CASE WHEN r.issue_id IS NOT NULL THEN (r.project_id::bigint << 32) + r.issue_id END AS issue_key,
       r.issue_id AS source_issue_id, r.event_id,
       to_timestamp(r.timestamp) AS event_at, r.received_at AT TIME ZONE 'UTC' AS received_at,
       p.value AS platform, e.value AS environment, av.value AS app_version,
       ab.value AS app_build, an.value AS app_name, os.value AS os_name,
       osv.value AS os_version, m.value AS device_model, nullif(u.value,'') AS identity,
       et.value AS exception_type, em.value AS message,
       coalesce(nullif(em.value, ''), nullif(et.value, ''), 'Untitled event') AS title,
       s.frames AS stack_frames,
       coalesce(tags.values, '{}'::jsonb) AS tags,
       coalesce(contexts.values, '{}'::jsonb) AS custom_contexts,
       coalesce(nullif(tags.values->>'layer',''), nullif(p.value,''), 'Unclassified') AS layer,
       coalesce(nullif(tags.values->>'component',''), 'Unclassified') AS component,
       contexts.values #>> '{trace,trace_id}' AS trace_id,
       r.session_id
FROM public.report r
LEFT JOIN public.unwrap_platform p ON p.id=r.platform_id
LEFT JOIN public.unwrap_environment e ON e.id=r.environment_id
LEFT JOIN public.unwrap_app_version av ON av.id=r.app_version_id
LEFT JOIN public.unwrap_app_build ab ON ab.id=r.app_build_id
LEFT JOIN public.unwrap_app_name an ON an.id=r.app_name_id
LEFT JOIN public.unwrap_os_name os ON os.id=r.os_name_id
LEFT JOIN public.unwrap_os_version osv ON osv.id=r.os_version_id
LEFT JOIN public.unwrap_model m ON m.id=r.model_id
LEFT JOIN public.unwrap_user u ON u.id=r.user_id
LEFT JOIN public.unwrap_exception_type et ON et.id=r.exception_type_id
LEFT JOIN public.unwrap_exception_message em ON em.id=r.exception_message_id
LEFT JOIN public.unwrap_stacktrace s ON s.id=r.stacktrace_id
LEFT JOIN LATERAL (
    SELECT jsonb_object_agg(k.value,v.value) AS values
    FROM public.report_tag t
    JOIN public.unwrap_tag_key k ON k.id=t.key_id
    JOIN public.unwrap_tag_value v ON v.id=t.value_id
    WHERE t.report_id=r.id
) tags ON true
LEFT JOIN LATERAL (
    SELECT jsonb_object_agg(k.value,v.value::jsonb) AS values
    FROM public.report_context c
    JOIN public.unwrap_context_key k ON k.id=c.key_id
    JOIN public.unwrap_context_value v ON v.id=c.value_id
    WHERE c.report_id=r.id
) contexts ON true;

CREATE OR REPLACE VIEW crash_cache_explorer.issues AS
SELECT issue_key AS id, project_id, source_issue_id,
       (array_agg(title ORDER BY event_at DESC,id DESC))[1] AS title,
       (array_agg(exception_type ORDER BY event_at DESC,id DESC))[1] AS exception_type,
       min(event_at) AS first_seen, max(event_at) AS last_seen,
       count(*) AS event_count, count(DISTINCT nullif(identity,'')) AS affected_identities,
       count(*) FILTER (WHERE nullif(identity,'') IS NULL) AS reports_without_identity,
       array_agg(DISTINCT platform) FILTER (WHERE platform IS NOT NULL) AS platforms,
       array_agg(DISTINCT environment) FILTER (WHERE environment IS NOT NULL) AS environments,
       array_agg(DISTINCT app_version) FILTER (WHERE app_version IS NOT NULL) AS versions,
       (array_agg(id ORDER BY event_at DESC,id DESC))[1] AS latest_report_id
FROM crash_cache_explorer.reports
WHERE issue_key IS NOT NULL
GROUP BY issue_key,project_id,source_issue_id;

CREATE OR REPLACE VIEW crash_cache_explorer.frames AS
SELECT (r.id::bigint << 32) + f.position AS id, r.id AS report_id,
       r.project_id, r.issue_key, f.position,
       f.frame->>'function' AS function, f.frame->>'module' AS module,
       coalesce(f.frame->>'filename', f.frame->>'abs_path') AS filename,
       f.frame->>'lineno' AS line_number, f.frame->>'in_app' AS in_app,
       f.frame->>'context_line' AS source_line,
       f.frame->'pre_context' AS source_before, f.frame->'post_context' AS source_after,
       f.frame->'vars' AS variables, f.frame AS frame
FROM crash_cache_explorer.reports r
CROSS JOIN LATERAL jsonb_array_elements(
    CASE WHEN jsonb_typeof(r.stack_frames)='array' THEN r.stack_frames ELSE '[]'::jsonb END
) WITH ORDINALITY AS f(frame,position);

CREATE OR REPLACE VIEW crash_cache_explorer.breadcrumbs AS
SELECT (r.id::bigint << 32) + b.seq AS id, r.id AS report_id, r.project_id,
       r.issue_key, b.seq AS sequence,
       row_number() OVER (PARTITION BY r.id ORDER BY c.timestamp NULLS LAST,b.seq) AS chronology,
       to_timestamp(c.timestamp / 1000.0) AS event_at,
       category.value AS category, kind.value AS type, level.value AS level,
       c.data->>'message' AS message, c.data
FROM public.report_breadcrumb b
JOIN crash_cache_explorer.reports r ON r.id=b.report_id
JOIN public.unwrap_breadcrumb c ON c.id=b.breadcrumb_id
LEFT JOIN public.unwrap_breadcrumb_category category ON category.id=c.category_id
LEFT JOIN public.unwrap_breadcrumb_type kind ON kind.id=c.type_id
LEFT JOIN public.unwrap_breadcrumb_level level ON level.id=c.level_id;

CREATE OR REPLACE VIEW crash_cache_explorer.attachments AS
SELECT a.id, a.project_id, r.id AS report_id, r.issue_key, a.event_id,
       a.filename, a.attachment_type, a.content_type, a.size_bytes,
       a.created_at AT TIME ZONE 'UTC' AS received_at
FROM public.attachment_metadata a
LEFT JOIN crash_cache_explorer.reports r
       ON r.project_id=a.project_id AND r.event_id=a.event_id;

CREATE OR REPLACE VIEW crash_cache_explorer.logs AS
SELECT l.id, l.project_id, to_timestamp(l.timestamp) AS event_at,
       l.trace_id, l.span_id, l.level, l.body, l.severity_number, l.attributes
FROM public.telemetry_log l;

-- A trace can contain several reports: this relation deliberately has a composite
-- (report_id,log_id) identity; neither column alone is a primary key. No time-only
-- or device-only correlation is inferred. Uncorrelated logs remain in logs above.
CREATE OR REPLACE VIEW crash_cache_explorer.report_logs AS
SELECT r.project_id, r.id AS report_id, r.issue_key, l.id AS log_id
FROM crash_cache_explorer.reports r
JOIN crash_cache_explorer.logs l ON l.project_id=r.project_id AND l.trace_id=r.trace_id
WHERE r.trace_id ~ '^[0-9a-fA-F]{32}$' AND r.trace_id !~ '^0{32}$';
CREATE OR REPLACE VIEW crash_cache_explorer.sessions AS
SELECT s.id, s.project_id, s.sid, s.distinct_id AS identity,
       s.started_at::timestamptz AS started_at, s.timestamp::timestamptz AS updated_at,
       status.value AS status, release.value AS release, environment.value AS environment,
       s.errors, s.duration, s.abnormal_mechanism
FROM public.session s
JOIN public.unwrap_session_status status ON status.id=s.status_id
LEFT JOIN public.unwrap_session_release release ON release.id=s.release_id
LEFT JOIN public.unwrap_session_environment environment ON environment.id=s.environment_id;
COMMIT;
