"""Aggregate-only diagnostic evidence charts; no raw diagnostic content is exposed."""

from dashboard_charts import BLUE, CORAL, PURPLE, TEAL


ERRORS = """SELECT r.* FROM report r
WHERE r.project_id = {{project_id}} AND r.issue_id IS NOT NULL
  AND to_timestamp(r.timestamp) >= {{from}}::timestamptz
  AND to_timestamp(r.timestamp) < {{until}}::timestamptz"""


def specifications():
    """Return independent diagnostic-dashboard cards using native date/project tags."""
    prefix = f"WITH selected AS ({ERRORS})"
    breadcrumb_coverage = prefix + """, evidence AS (
    SELECT r.id,
      coalesce(nullif((SELECT tv.value
                FROM report_tag t JOIN unwrap_tag_key tk ON tk.id=t.key_id
                JOIN unwrap_tag_value tv ON tv.id=t.value_id
                WHERE t.report_id=r.id AND tk.value='layer'), ''),
               (SELECT p.value FROM unwrap_platform p WHERE p.id=r.platform_id), 'Unclassified') AS layer,
      EXISTS (SELECT 1 FROM report_breadcrumb b WHERE b.report_id=r.id) AS has_breadcrumbs
    FROM selected r
)
SELECT layer, count(*) FILTER (WHERE has_breadcrumbs) AS with_breadcrumbs,
       count(*) FILTER (WHERE NOT has_breadcrumbs) AS without_breadcrumbs
FROM evidence GROUP BY layer ORDER BY layer"""
    categories = prefix + """, categorized AS (
    SELECT r.id, coalesce(c.value, 'Unclassified') AS category
    FROM selected r JOIN report_breadcrumb rb ON rb.report_id=r.id
    JOIN unwrap_breadcrumb b ON b.id=rb.breadcrumb_id
    LEFT JOIN unwrap_breadcrumb_category c ON c.id=b.category_id
)
SELECT category, count(DISTINCT id) AS reports_with_category
FROM categorized GROUP BY category ORDER BY reports_with_category DESC, category"""
    frames = prefix + """, evidence AS (
    SELECT r.id,
      EXISTS (SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof(s.frames)='array' THEN s.frames ELSE '[]'::jsonb END) f
              WHERE jsonb_typeof(f)='object') AS stack,
      EXISTS (SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof(s.frames)='array' THEN s.frames ELSE '[]'::jsonb END) f
              WHERE jsonb_typeof(f->'function')='string' AND length(f->>'function')>0) AS named_function,
      EXISTS (SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof(s.frames)='array' THEN s.frames ELSE '[]'::jsonb END) f
              WHERE (jsonb_typeof(f->'context_line')='string' AND length(f->>'context_line')>0)
                 OR (jsonb_typeof(f->'pre_context')='array' AND f->'pre_context'<>'[]'::jsonb)
                 OR (jsonb_typeof(f->'post_context')='array' AND f->'post_context'<>'[]'::jsonb)) AS source_context,
      EXISTS (SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof(s.frames)='array' THEN s.frames ELSE '[]'::jsonb END) f
              WHERE jsonb_typeof(f->'vars')='object' AND f->'vars'<>'{}'::jsonb) AS variables
    FROM selected r LEFT JOIN unwrap_stacktrace s ON s.id=r.stacktrace_id
)
SELECT metric.label AS evidence, count(*) FILTER (WHERE metric.present) AS present,
       count(*) FILTER (WHERE NOT metric.present) AS absent
FROM evidence CROSS JOIN LATERAL (VALUES ('Stack frames',stack),('Named function',named_function),
       ('Source context',source_context),('Variables',variables)) AS metric(label,present)
GROUP BY metric.label ORDER BY metric.label"""
    symbolication = prefix + """, outcomes AS (
    SELECT CASE WHEN c.status IN ('complete','partial','missing_symbols') THEN c.status
                WHEN c.status IS NULL THEN 'Not recorded' ELSE 'Other' END AS status
    FROM selected r LEFT JOIN LATERAL (
      SELECT cv.value::jsonb->>'status' AS status FROM report_context rc
      JOIN unwrap_context_key ck ON ck.id=rc.key_id
      JOIN unwrap_context_value cv ON cv.id=rc.value_id
      WHERE rc.report_id=r.id AND ck.value='symbolication'
    ) c ON true
)
SELECT status, count(*) AS reports FROM outcomes GROUP BY status ORDER BY status"""
    contexts = prefix + """, categorized AS (
    SELECT r.id, k.value AS context_family
    FROM selected r JOIN report_context c ON c.report_id=r.id
    JOIN unwrap_context_key k ON k.id=c.key_id
)
SELECT context_family, count(DISTINCT id) AS reports_with_context
FROM categorized GROUP BY context_family ORDER BY reports_with_context DESC, context_family"""

    def card(name, query, dimension, metrics, description, layout, colors):
        return {"name": name, "description": description, "query": query, "display": "row",
                "settings": {"graph.dimensions": [dimension], "graph.metrics": metrics,
                             "graph.colors": colors, "graph.show_values": True,
                             "graph.y_axis.auto_range": True, "graph.y_axis.min": 0,
                             "stackable.stack_type": "stacked"}, "layout": layout}

    return [
        {"name": "Diagnostic error reports", "description": "Stored reports linked to an issue in the selected project and date range. Activity events are excluded; throttled or undelivered reports are not counted.",
         "query": prefix + " SELECT count(*) AS error_reports FROM selected", "display": "scalar",
         "settings": {"scalar.field": "error_reports", "scalar.decimals": 0}, "layout": (0, 0, 24, 4)},
        card("Breadcrumb coverage by layer", breadcrumb_coverage, "layer", ["with_breadcrumbs", "without_breadcrumbs"],
             "Stored error reports with or without at least one breadcrumb. Missing evidence can reflect client support, configuration or capture timing.", (4, 0, 12, 8), [TEAL, CORAL]),
        card("Breadcrumb categories in reports", categories, "category", ["reports_with_category"],
             "Distinct reports containing each category; repeated and shared breadcrumbs do not multiply reports. Categories overlap and cannot be summed. Custom category names are retained.", (4, 12, 12, 8), [BLUE]),
        card("Stack diagnostic evidence", frames, "evidence", ["present", "absent"],
             "Presence of stored frame objects, function names, source context and nonempty variable objects per error report. Evidence categories overlap; presence does not establish stack accuracy or completeness.", (12, 0, 12, 9), [TEAL, CORAL]),
        card("Native symbolication outcomes", symbolication, "status", ["reports"],
             "Backend-recorded symbolication status. Not recorded includes ordinary managed errors and is not a failed symbolication. Complete refers to the processed stack, not every frame in the crashed process.", (12, 12, 12, 9), [PURPLE]),
        card("Stored custom context families", contexts, "context_family", ["reports_with_context"],
             "Distinct error reports per stored custom context family. Built-in device, OS and app fields are normalized elsewhere. Custom family names are retained; values are never displayed. Families overlap.", (21, 0, 24, 8), [BLUE]),
    ]


def pipeline_specifications():
    """Independent ingestion, structured-log and attachment-metadata dashboard."""
    archives = """WITH received AS (
 SELECT a.* FROM archive a WHERE a.project_id={{project_id}}
 AND a.created_at AT TIME ZONE 'UTC' >= {{from}}::timestamptz
 AND a.created_at AT TIME ZONE 'UTC' < {{until}}::timestamptz
)"""
    logs = """WITH logs AS (
 SELECT timestamp, CASE WHEN level IN ('trace','debug','info','warn','error','fatal')
                       THEN level ELSE 'Other' END AS severity
 FROM telemetry_log WHERE project_id={{project_id}}
 AND to_timestamp(timestamp) >= {{from}}::timestamptz
 AND to_timestamp(timestamp) < {{until}}::timestamptz
)"""
    attachments = archives + """, attachments AS (
 SELECT m.size_bytes,
 CASE WHEN m.attachment_type='event.minidump' THEN 'Native crash dump'
      WHEN m.attachment_type='event.applecrashreport' THEN 'Apple crash report'
      WHEN m.attachment_type='event.view_hierarchy' THEN 'View hierarchy'
      WHEN (m.attachment_type='event.attachment' OR m.attachment_type IS NULL)
           AND ((m.filename='screenshot.png' AND m.content_type='image/png')
                OR (m.filename IN ('screenshot.jpg','screenshot.jpeg') AND m.content_type='image/jpeg')) THEN 'Screenshot'
      WHEN (m.attachment_type='event.attachment' OR m.attachment_type IS NULL)
           AND m.filename='godot.log' THEN 'Application log'
      WHEN m.attachment_type='event.attachment' THEN 'Attachment'
      WHEN m.attachment_type IS NULL THEN 'Unspecified' ELSE 'Other' END AS attachment_type
 FROM attachment_metadata m JOIN received a ON a.hash=m.archive_hash AND a.project_id=m.project_id
)"""

    def scalar(name, query, field, description, layout):
        return {"name": name, "query": query, "description": description, "display": "scalar",
                "settings": {"scalar.field": field, "scalar.decimals": 0}, "layout": layout}

    def graph(name, query, dimension, metrics, description, layout, display="row"):
        return {"name": name, "query": query, "description": description, "display": display,
                "settings": {"graph.dimensions": [dimension], "graph.metrics": metrics,
                             "graph.colors": [TEAL, BLUE, PURPLE, CORAL, "#E9B44C", "#A0A0A0", "#505050"],
                             "graph.show_values": display == "row", "graph.y_axis.min": 0,
                             "graph.y_axis.auto_range": True, "stackable.stack_type": "stacked",
                             "graph.x_axis.scale": "ordinal"}, "layout": layout}

    return [
        scalar("Received archives", archives + " SELECT count(*) AS archives FROM received", "archives",
               "Stored distinct archives received within the selected dates. Archive deduplication means this is not an HTTP request count.", (0, 0, 8, 4)),
        scalar("Current pending archives", archives + " SELECT count(*) AS pending FROM received a WHERE EXISTS (SELECT 1 FROM queue q WHERE q.archive_hash=a.hash)", "pending",
               "Current queue entries among archives received in the selected dates. This is a present snapshot, not historical backlog; older queued archives outside the date range are excluded.", (0, 8, 8, 4)),
        scalar("Current failed archives", archives + " SELECT count(*) AS failed FROM received a WHERE EXISTS (SELECT 1 FROM queue_error q WHERE q.archive_hash=a.hash)", "failed",
               "Current error-queue entries among archives received in the selected dates. Retries can change this value. Raw error messages are not exposed.", (0, 16, 8, 4)),
        graph("Structured logs by severity", logs + " SELECT severity, count(*) AS logs FROM logs GROUP BY severity ORDER BY logs DESC, severity",
              "severity", ["logs"], "Persisted structured logs by reported event time, not breadcrumbs. Missing or throttled logs are not counted. Unrecognized severity values are grouped as Other.", (4, 0, 12, 8)),
        graph("Structured logs by UTC day", logs + """ SELECT to_char(to_timestamp(timestamp) AT TIME ZONE 'UTC','YYYY-MM-DD') AS day,
 count(*) FILTER (WHERE severity='trace') AS trace, count(*) FILTER (WHERE severity='debug') AS debug,
 count(*) FILTER (WHERE severity='info') AS info, count(*) FILTER (WHERE severity='warn') AS warn,
 count(*) FILTER (WHERE severity='error') AS error, count(*) FILTER (WHERE severity='fatal') AS fatal,
 count(*) FILTER (WHERE severity='Other') AS other
 FROM logs GROUP BY day ORDER BY day""", "day", ["trace", "debug", "info", "warn", "error", "fatal", "other"],
              "Stored log records per UTC day and severity, using reported event time. Boundary days are clipped to the selected range. Days without stored logs are absent, not proof of inactivity.", (4, 12, 12, 8), "bar"),
        graph("Received attachment types", attachments + " SELECT attachment_type, count(*) AS attachments FROM attachments GROUP BY attachment_type ORDER BY attachments DESC, attachment_type",
              "attachment_type", ["attachments"], "Attachment metadata from archives received in the selected dates, including unattached items. Counts refer to archived items, not unique content. Screenshot classification requires the known screenshot filename and matching image MIME type; Application log identifies godot.log. These metadata labels do not prove valid content.", (12, 0, 12, 8)),
        {**graph("Received attachment size (KiB)", attachments + " SELECT attachment_type, round(sum(size_bytes) / 1024.0, 3) AS payload_kib FROM attachments GROUP BY attachment_type ORDER BY payload_kib DESC, attachment_type",
              "attachment_type", ["payload_kib"], "Original attachment payload size in KiB (1 KiB = 1,024 bytes), rounded to three decimals. Excludes envelope and database overhead; this is not compressed disk usage. Raw filenames and content are not displayed.", (12, 12, 12, 8)), "previous_name": "Received attachment bytes"},
        graph("Error reports with attachment metadata", f"WITH selected AS ({ERRORS}), evidence AS (" + """
 SELECT EXISTS (SELECT 1 FROM attachment_metadata m WHERE m.project_id=r.project_id
                AND m.event_id=r.event_id) AS attached FROM selected r
) SELECT CASE WHEN attached THEN 'With metadata' ELSE 'Without metadata' END AS coverage,
 count(*) AS reports FROM evidence GROUP BY attached ORDER BY coverage""", "coverage", ["reports"],
              "Error reports in the selected event-time range with at least one matching project/event attachment record. Multiple attachments count once. Metadata is checked regardless of arrival time; missing metadata does not prove the client never captured an attachment.", (20, 0, 24, 8)),
    ]
