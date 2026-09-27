"""Chart specifications for the supported Metabase dashboard API."""

TEAL = "#168B83"
BLUE = "#509EE3"
CORAL = "#E76F51"
PURPLE = "#8B6BB1"
AMBER = "#E9B44C"


GRAINS = ("hour", "day", "week", "month")


def period_query(query, metric):
    """Recompute distinct counts from observations for the selected calendar period."""
    source = query.replace("('day'), ('week'), ('month')",
                           "(CASE WHEN {{grain}} IN ('hour', 'day', 'week', 'month') THEN {{grain}} ELSE NULL END)")
    # A single-day hourly view uses compact ticks; the date remains in its filter.
    # Longer ranges retain the date to prevent ambiguous repeated hour labels.
    return f"""WITH source AS ({source})
SELECT CASE WHEN {{{{grain}}}} = 'hour' AND {{{{until}}}}::date - {{{{from}}}}::date = 1
            THEN to_char(period_start, 'HH24:MI') || ' UTC'
            ELSE to_char(period_start, 'YYYY-MM-DD HH24:MI') || ' UTC' END AS period_start,
       {metric}
FROM source WHERE grain IS NOT NULL ORDER BY source.period_start"""


def device_query(query):
    """Count identity before dropping app/runtime/OS-version dimensions."""
    observation = query[:query.index("SELECT platform.value AS runtime_platform")]
    return observation + """SELECT concat(coalesce(model.value, 'Unknown device'), ' / ',
       coalesce(os.value, 'Unknown OS')) AS device,
       coalesce(model.value, 'Unknown device') AS device_model,
       count(DISTINCT u.value) AS observed_installations
FROM observation r
JOIN unwrap_user u ON u.id = r.user_id
LEFT JOIN unwrap_os_name os ON os.id = r.os_name_id
LEFT JOIN unwrap_model model ON model.id = r.model_id
GROUP BY model.value, os.value
ORDER BY observed_installations DESC, device"""


def specifications(queries):
    sessions, activity, health, devices, errors, _duplicates = queries

    def wrap(query, select):
        return f"WITH source AS ({query}) {select}"

    def graph(dimension, metrics, colors, **extra):
        return {"graph.dimensions": [dimension], "graph.metrics": metrics,
                "graph.colors": colors, "graph.show_values": False,
                "graph.y_axis.auto_range": True, "graph.y_axis.min": 0,
                "graph.show_goal": False, **extra}

    # Count installation identity over the whole selected range, not summed DAU.
    installations = activity[:activity.index("SELECT period.grain")]
    installations += "SELECT count(DISTINCT installation) AS active_installations FROM activity"
    return [
        {"name": "Observed installations", "description": "Distinct installations with foreground activity in this date range; installations are not people.",
         "query": installations, "display": "scalar", "settings": {"scalar.field": "active_installations", "scalar.decimals": 0}, "layout": (0, 0, 8, 4)},
        {"name": "Sessions started", "description": "Recorded session starts in this date range, across the configured clients.",
         "query": wrap(sessions, "SELECT coalesce(sum(sessions_started), 0) AS sessions FROM source WHERE grain = 'day'"),
         "display": "scalar", "settings": {"scalar.field": "sessions", "scalar.decimals": 0}, "layout": (0, 8, 8, 4)},
        {"name": "Reported crashed sessions", "description": "Sessions explicitly classified crashed. Open or missing terminal sessions are not proof of health.",
         "query": wrap(health, "SELECT coalesce(sum(crashes), 0) AS crashed_sessions FROM source"),
         "display": "scalar", "settings": {"scalar.field": "crashed_sessions", "scalar.decimals": 0}, "layout": (0, 16, 8, 4)},
        {"name": "Observed activity by period", "previous_name": "Activity through the day", "description": "Distinct installations per selected UTC calendar period, recomputed from activity rather than summed daily counts. Partial boundary periods contain only the selected date range. Missing observations do not prove inactivity.",
         "query": period_query(activity, "observed_active_installations"), "display": "line",
         "settings": graph("period_start", ["observed_active_installations"], [TEAL], **{"graph.show_dots": True, "graph.x_axis.title_text": "Period start (UTC)", "graph.y_axis.title_text": "Observed installations", "line.interpolate": "linear", "graph.x_axis.scale": "ordinal"}), "layout": (4, 0, 12, 7)},
        {"name": "Session starts by period", "previous_name": "Session starts through the day", "description": "Session starts per selected UTC calendar period, across the configured SDKs. Boundary periods are clipped to the date range.",
         "query": period_query(sessions, "sessions_started"), "display": "bar",
         "settings": graph("period_start", ["sessions_started"], [BLUE], **{"graph.x_axis.title_text": "Period start (UTC)", "graph.y_axis.title_text": "Sessions", "graph.x_axis.scale": "ordinal"}), "layout": (4, 12, 12, 7)},
        {"name": "Session outcomes by release", "description": "Recorded outcomes; open and abnormal sessions remain visible instead of being called healthy.",
         "query": wrap(health, "SELECT concat(regexp_replace(split_part(release, '+', 1), '^.*@', ''), ' / ', environment) AS build, sum(sessions - crashes - abnormal - unhandled - still_open) AS exited, sum(crashes) AS crashes, sum(abnormal) AS abnormal, sum(unhandled) AS unhandled, sum(still_open) AS still_open FROM source GROUP BY build ORDER BY build"),
         "display": "bar", "settings": graph("build", ["exited", "crashes", "abnormal", "unhandled", "still_open"], [TEAL, CORAL, AMBER, PURPLE, BLUE], **{"stackable.stack_type": "stacked", "graph.x_axis.title_text": "Release / environment", "graph.y_axis.title_text": "Sessions", "graph.label_value_formatting": "compact"}), "layout": (11, 0, 12, 8)},
        {"name": "Error reports by capture layer", "description": "Report volume, not an error rate. Reports reflect the configured sampling and capture policies.",
         "query": wrap(errors, "SELECT coalesce(layer, 'Unclassified') AS layer, sum(error_events) AS reports FROM source GROUP BY layer ORDER BY reports DESC"),
         "display": "bar", "settings": graph("layer", ["reports"], [CORAL], **{"graph.show_values": True, "graph.x_axis.title_text": "Capture layer", "graph.y_axis.title_text": "Reports"}), "layout": (11, 12, 12, 8)},
        {"name": "Observed devices and systems", "description": "Distinct installations per device model and OS across the selected range. App, OS-version and runtime changes do not multiply the count. A changed model or OS name can put one installation in multiple bars; do not sum bars. The detail table retains versions.",
         "query": device_query(devices),
         "display": "row", "settings": graph("device", ["observed_installations"], [PURPLE], **{"graph.show_values": True}), "layout": (19, 0, 24, 7)},
    ]
