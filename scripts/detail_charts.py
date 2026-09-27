"""Filtered report-level diagnostics without raw event payloads or identifiers."""

from dashboard_charts import BLUE, CORAL, TEAL


FILTERS = ("layer", "app_version", "device_model")


def selected_reports():
    """Use report-owned metadata; the three text parameters default to All."""
    return """WITH normalized AS (
 SELECT r.id, r.timestamp, r.stacktrace_id,
 coalesce(nullif((SELECT v.value FROM report_tag t
           JOIN unwrap_tag_key k ON k.id=t.key_id JOIN unwrap_tag_value v ON v.id=t.value_id
           WHERE t.report_id=r.id AND k.value='layer'), ''), p.value, 'Unclassified') AS layer,
 coalesce(nullif((SELECT v.value FROM report_tag t
           JOIN unwrap_tag_key k ON k.id=t.key_id JOIN unwrap_tag_value v ON v.id=t.value_id
           WHERE t.report_id=r.id AND k.value='component'), ''), 'Unclassified') AS component,
 coalesce(p.value,'Unknown runtime') AS runtime,
 coalesce(a.value,'Unknown version') AS app_version,
 coalesce(m.value,'Unknown device') AS device_model,
 coalesce(e.value, 'Unclassified') AS exception_type
 FROM report r
 LEFT JOIN unwrap_platform p ON p.id=r.platform_id
 LEFT JOIN unwrap_app_version a ON a.id=r.app_version_id
 LEFT JOIN unwrap_model m ON m.id=r.model_id
 LEFT JOIN unwrap_exception_type e ON e.id=r.exception_type_id
 WHERE r.project_id={{project_id}} AND r.issue_id IS NOT NULL
   AND to_timestamp(r.timestamp)>={{from}}::timestamptz
   AND to_timestamp(r.timestamp)<{{until}}::timestamptz
), selected AS (
 SELECT * FROM normalized WHERE ({{layer}}='All' OR layer={{layer}})
   AND ({{app_version}}='All' OR app_version={{app_version}})
   AND ({{device_model}}='All' OR device_model={{device_model}})
)"""


def specifications():
    prefix = selected_reports()

    def graph(name, query, dimension, description, layout, display="row"):
        return {"name": name, "description": description, "query": prefix + query,
                "display": display, "settings": {"graph.dimensions": [dimension],
                "graph.metrics": ["reports"], "graph.colors": [TEAL, BLUE, CORAL],
                "graph.show_values": display == "row", "graph.y_axis.min": 0,
                "graph.y_axis.auto_range": True, "graph.x_axis.scale": "ordinal"}, "layout": layout}

    return [
        graph("Selected error reports by day", " SELECT to_char(to_timestamp(timestamp) AT TIME ZONE 'UTC','YYYY-MM-DD') AS day, count(*) AS reports FROM selected GROUP BY day ORDER BY day",
              "day", "Error report counts by UTC event date, filtered by their own capture layer, version and device. Counts are not population error rates; days with no reports are omitted.", (0, 0, 24, 7), "bar"),
        graph("Selected error components", " SELECT component, count(*) AS reports FROM selected GROUP BY component ORDER BY reports DESC, component",
              "component", "Components declared on the selected error reports. Custom component names are retained; missing classification remains visible.", (7, 0, 12, 8)),
        graph("Selected exception types", " SELECT exception_type, count(*) AS reports FROM selected GROUP BY exception_type ORDER BY reports DESC, exception_type",
              "exception_type", "Exception class names and native signals from any SDK. Exception messages are excluded.", (7, 12, 12, 8)),
        {"name": "Selected report summaries", "description": "Latest 200 matching reports, using each report's own metadata. No event IDs, messages, source, variables or breadcrumb content are exposed. Similar-looking rows may represent separate reports. Missing evidence is not a proof that capture is unsupported.",
         "query": prefix + """ SELECT to_char(to_timestamp(r.timestamp) AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS') AS event_time_utc,
 r.layer, r.component, r.runtime, r.app_version, r.device_model, r.exception_type,
 EXISTS (SELECT 1 FROM report_breadcrumb b WHERE b.report_id=r.id) AS has_breadcrumbs,
 EXISTS (SELECT 1 FROM unwrap_stacktrace s CROSS JOIN LATERAL
         jsonb_array_elements(CASE WHEN jsonb_typeof(s.frames)='array' THEN s.frames ELSE '[]'::jsonb END) f
         WHERE s.id=r.stacktrace_id AND jsonb_typeof(f)='object') AS has_frames
 FROM selected r ORDER BY r.timestamp DESC, r.id DESC LIMIT 200""",
         "display": "table", "settings": {}, "layout": (15, 0, 24, 12)},
    ]
