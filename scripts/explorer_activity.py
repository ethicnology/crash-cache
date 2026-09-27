"""Read-only audience panels based on explicit activity and Sentry sessions.

An installation is a nonempty client identity, not a person or an app-store
installation. First observed means first retained activity in this project.
Sessions are starts, not concurrent players. Their platform is correlated only
through project, installation identity and an exact canonical app-session UUID.
"""

UUID_PATTERN = '^([0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$'

# OS is the useful audience dimension; SDK platform remains a labelled fallback.
PLATFORM = "coalesce(nullif(os_name,''), 'Unknown system (' || nullif(platform,'') || ')', 'Unknown')"

COMMON = '''WITH parameters AS (
 SELECT {{project_id}}::integer AS project_id,
 ({{from}}::date::timestamp AT TIME ZONE 'UTC') AS since, ({{until}}::date::timestamp AT TIME ZONE 'UTC') AS until,
 CASE {{grain}} WHEN 'hour' THEN 'hour' WHEN 'day' THEN 'day'
 WHEN 'week' THEN 'week' WHEN 'month' THEN 'month' END AS grain
), observations AS (
 SELECT r.id,r.project_id,r.event_at,nullif(r.identity,'') AS identity,
 PLATFORM_EXPRESSION AS platform,
 CASE WHEN r.tags->>'app_session_id' ~* 'UUID_PATTERN'
 THEN replace(lower(r.tags->>'app_session_id'),'-','') END AS canonical_sid
 FROM crash_cache_explorer.reports r CROSS JOIN parameters p
 WHERE r.project_id=p.project_id
 AND r.tags->>'event_kind' IN ('app_session','app_activity')
), activity AS (
 SELECT o.* FROM observations o CROSS JOIN parameters p
 WHERE o.event_at>=p.since AND o.event_at<p.until AND p.grain IS NOT NULL
), session_starts AS (
 SELECT s.*,coalesce(c.platform,'Unknown') AS platform,
 coalesce(c.platform_count,0) AS correlated_platforms
 FROM crash_cache_explorer.sessions s CROSS JOIN parameters p
 LEFT JOIN LATERAL (
  SELECT CASE WHEN count(DISTINCT o.platform)=1 THEN min(o.platform) END AS platform,
  count(DISTINCT o.platform) AS platform_count
  FROM observations o WHERE o.project_id=s.project_id
  AND o.identity=nullif(s.identity,'')
  AND o.canonical_sid=replace(lower(s.sid),'-','')
 ) c ON true
 WHERE s.project_id=p.project_id AND s.started_at>=p.since
 AND s.started_at<p.until AND p.grain IS NOT NULL
)
'''.replace('PLATFORM_EXPRESSION', PLATFORM).replace('UUID_PATTERN', UUID_PATTERN)


def chart_settings(x, y, series=None):
    """Explicit axis mapping avoids Metabase guessing a numeric series key."""
    return {'graph.dimensions': [x] + ([series] if series else []),
            'graph.metrics': [y], 'graph.show_values': False,
            'graph.x_axis.title_text': x.replace('_', ' ').capitalize(),
            'graph.y_axis.title_text': y.replace('_', ' ').capitalize()}


def definitions():
    """Return name, native SQL, display, visualization settings and grid layout.

    Required parameters: project_id (number), from/until (date), grain (text,
    hour/day/week/month). Bounds are half-open UTC instants. Unknown grains
    produce no observations instead of being interpolated into SQL functions.
    """
    return [
        ('Active installations in period', COMMON + '''SELECT count(DISTINCT identity) AS active_installations
FROM activity''', 'scalar', {}, (3, 0, 6, 4)),
        ('First observed installations in period', COMMON + '''SELECT count(*) AS first_observed_installations
FROM (SELECT identity,min(event_at) AS first_seen FROM observations
WHERE identity IS NOT NULL GROUP BY identity) first_seen CROSS JOIN parameters p
WHERE first_seen>=p.since AND first_seen<p.until AND p.grain IS NOT NULL''',
         'scalar', {}, (3, 6, 6, 4)),
        ('Sessions started in period', COMMON + '''SELECT count(*) AS sessions_started
FROM session_starts''', 'scalar', {}, (3, 12, 6, 4)),
        ('Started sessions still open', COMMON + '''SELECT count(*) AS sessions_still_open
FROM session_starts WHERE status='ok' ''', 'scalar', {}, (3, 18, 6, 4)),
        ('Active installations over time', COMMON + '''SELECT
 date_trunc(p.grain,a.event_at AT TIME ZONE 'UTC') AS period_start,
 count(DISTINCT a.identity) AS active_installations
FROM activity a CROSS JOIN parameters p GROUP BY period_start ORDER BY period_start''',
         'line', chart_settings('period_start', 'active_installations'), (7, 0, 12, 7)),
        ('Sessions started over time by system', COMMON + '''SELECT
 date_trunc(p.grain,s.started_at AT TIME ZONE 'UTC') AS period_start,s.platform AS system,
 count(*) AS sessions_started FROM session_starts s CROSS JOIN parameters p
GROUP BY period_start,s.platform ORDER BY period_start,s.platform''',
         'bar', {**chart_settings('period_start', 'sessions_started', 'system'), 'stackable.stack_type': 'stacked'}, (7, 12, 12, 7)),
        ('Active installations by system', COMMON + '''SELECT platform AS system,
 count(DISTINCT identity) AS active_installations
FROM activity GROUP BY platform ORDER BY active_installations DESC,system''',
         'bar', chart_settings('system', 'active_installations'), (14, 0, 12, 7)),
        ('Sessions started by system', COMMON + '''SELECT platform AS system,
 count(*) AS sessions_started FROM session_starts
GROUP BY platform ORDER BY sessions_started DESC,system''',
         'bar', chart_settings('system', 'sessions_started'), (14, 12, 12, 7)),
        ('Active installations over time by system', COMMON + '''SELECT
 date_trunc(p.grain,a.event_at AT TIME ZONE 'UTC') AS period_start,a.platform AS system,
 count(DISTINCT a.identity) AS active_installations
FROM activity a CROSS JOIN parameters p
GROUP BY period_start,a.platform ORDER BY period_start,a.platform''',
         'line', chart_settings('period_start', 'active_installations', 'system'), (21, 0, 12, 7)),
        ('First observed installations over time', COMMON + '''SELECT
 date_trunc(p.grain,f.first_seen AT TIME ZONE 'UTC') AS period_start,
 count(*) AS first_observed_installations
FROM (SELECT identity,min(event_at) AS first_seen FROM observations
WHERE identity IS NOT NULL GROUP BY identity) f CROSS JOIN parameters p
WHERE f.first_seen>=p.since AND f.first_seen<p.until AND p.grain IS NOT NULL
GROUP BY period_start ORDER BY period_start''',
         'bar', chart_settings('period_start', 'first_observed_installations'), (21, 12, 12, 7)),
        ('Active installations by system version', COMMON + '''SELECT a.platform AS system,
coalesce(nullif(r.os_version,''),'Unknown') AS system_version,
count(DISTINCT a.identity) AS active_installations
FROM activity a JOIN crash_cache_explorer.reports r ON r.id=a.id AND r.project_id=a.project_id
GROUP BY a.platform,r.os_version ORDER BY active_installations DESC,system,system_version''',
         'table', {}, (35, 0, 12, 7)),
        ('Active installations by device model', COMMON + '''SELECT a.platform AS system,
coalesce(nullif(r.device_model,''),'Unknown') AS device_model,
count(DISTINCT a.identity) AS active_installations
FROM activity a JOIN crash_cache_explorer.reports r ON r.id=a.id AND r.project_id=a.project_id
GROUP BY a.platform,r.device_model ORDER BY active_installations DESC,system,device_model''',
         'table', {}, (35, 12, 12, 7)),
        ('Active installations by app version', COMMON + '''SELECT
coalesce(nullif(r.app_version,''),'Unknown') AS app_version,
count(DISTINCT a.identity) AS active_installations
FROM activity a JOIN crash_cache_explorer.reports r ON r.id=a.id AND r.project_id=a.project_id
GROUP BY r.app_version ORDER BY active_installations DESC,app_version''',
         'table', {}, (42, 0, 24, 7)),
        ('Session outcomes by start period', COMMON + '''SELECT
 date_trunc(p.grain,s.started_at AT TIME ZONE 'UTC') AS period_start,s.status,
 count(*) AS sessions FROM session_starts s CROSS JOIN parameters p
GROUP BY period_start,s.status ORDER BY period_start,s.status''',
         'bar', {**chart_settings('period_start', 'sessions', 'status'), 'stackable.stack_type': 'stacked'}, (28, 0, 12, 7)),
        ('Activity collection coverage', COMMON + '''SELECT
 (SELECT count(*) FROM activity) AS activity_observations,
 (SELECT count(*) FROM activity WHERE identity IS NULL) AS observations_without_identity,
 (SELECT count(*) FROM session_starts WHERE nullif(identity,'') IS NULL) AS sessions_without_identity,
 (SELECT count(*) FROM session_starts WHERE correlated_platforms=0) AS sessions_without_platform_match,
 (SELECT count(*) FROM session_starts WHERE correlated_platforms>1) AS sessions_with_conflicting_platforms,
 (SELECT count(*) FROM session_starts WHERE platform='Unknown' OR platform LIKE 'Unknown system (%') AS sessions_without_known_system''',
         'table', {}, (28, 12, 12, 7)),
    ]
