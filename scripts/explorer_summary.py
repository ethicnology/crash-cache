"""Read-only summaries with explicit selection windows and collection limitations."""
from explorer_activity import COMMON


COMPARISON = COMMON + ''', windows AS (
 SELECT 'Current' AS label,since AS lower_bound,until AS upper_bound
 FROM parameters WHERE since<until AND grain IS NOT NULL
 UNION ALL
 SELECT 'Previous',to_timestamp(2*extract(epoch FROM since)-extract(epoch FROM until)),since
 FROM parameters WHERE since<until AND grain IS NOT NULL
), counts AS (
 SELECT w.label,
 (SELECT count(DISTINCT identity) FROM observations o
  WHERE o.event_at>=w.lower_bound AND o.event_at<w.upper_bound) AS installations,
 (SELECT count(*) FROM crash_cache_explorer.sessions s CROSS JOIN parameters p
  WHERE s.project_id=p.project_id AND s.started_at>=w.lower_bound
  AND s.started_at<w.upper_bound) AS sessions,
 (SELECT count(*) FROM crash_cache_explorer.reports r CROSS JOIN parameters p
  WHERE r.project_id=p.project_id AND r.issue_key IS NOT NULL
  AND r.event_at>=w.lower_bound AND r.event_at<w.upper_bound) AS errors
 FROM windows w
), metrics AS (
 SELECT ordinal,metric,
 coalesce(max(value) FILTER (WHERE label='Current'),0) AS current_value,
 coalesce(max(value) FILTER (WHERE label='Previous'),0) AS previous_value
 FROM counts CROSS JOIN LATERAL (VALUES
 (1,'Active installations',installations),
 (2,'Sessions started',sessions),
 (3,'Error occurrences',errors)) m(ordinal,metric,value)
 GROUP BY ordinal,metric
)
SELECT metric AS "Metric",current_value AS "Current",previous_value AS "Previous",
 current_value-previous_value AS "Absolute change",
 round(100.0*(current_value-previous_value)/nullif(previous_value,0),2) AS "Change percent",
 CASE WHEN previous_value=0 AND current_value=0 THEN 'No previous observations'
 WHEN previous_value=0 THEN 'Newly observed' ELSE 'Comparable' END AS "Interpretation"
FROM metrics ORDER BY ordinal
'''

HEALTH_CTES = COMMON + ''', errors AS (
 SELECT r.* FROM crash_cache_explorer.reports r CROSS JOIN parameters p
 WHERE r.project_id=p.project_id AND r.issue_key IS NOT NULL
 AND r.event_at>=p.since AND r.event_at<p.until AND p.grain IS NOT NULL
), coverage AS (
 SELECT (SELECT count(*) FROM activity) AS observations,
 (SELECT count(*) FROM activity WHERE identity IS NULL) AS missing_identity,
 (SELECT count(*) FROM session_starts WHERE platform='Unknown'
   OR platform LIKE 'Unknown system (%') AS unknown_system,
 (SELECT count(*) FROM errors) AS error_count,
 (SELECT count(*) FROM errors WHERE CASE WHEN jsonb_typeof(stack_frames)='array'
   THEN jsonb_array_length(stack_frames)=0 ELSE true END) AS missing_stacks
)
'''

HEALTH = HEALTH_CTES + '''SELECT coalesce(p.since<p.until AND p.grain IS NOT NULL,false) AS "Valid period",
 p.since AS "Period start UTC",p.until AS "Period end UTC (exclusive)",
 CASE WHEN p.since IS NULL OR p.until IS NULL OR p.since>=p.until OR p.grain IS NULL THEN 'Invalid period selection'
 WHEN observations=0 THEN 'No activity observations received'
 WHEN missing_identity>0 THEN 'Some activity cannot be attributed to an installation'
 ELSE 'Activity identities available' END AS "Activity coverage",
 observations AS "Activity observations",missing_identity AS "Activity missing identity",
 unknown_system AS "Started sessions without known system",
 error_count AS "Error occurrences",missing_stacks AS "Errors without stack frames",
 'Missing activity does not establish zero players; missing stacks cannot be reconstructed here.' AS "Interpretation"
FROM coverage CROSS JOIN parameters p
'''


STATUS = HEALTH_CTES + '''SELECT CASE
 WHEN p.since IS NULL OR p.until IS NULL OR p.since>=p.until OR p.grain IS NULL
 THEN 'Choose a continuous date range'
 WHEN observations=0 AND error_count>0 THEN 'Errors received; no activity telemetry'
 WHEN observations=0 THEN 'No activity received for this selection'
 WHEN missing_identity>0 OR unknown_system>0 OR missing_stacks>0
 THEN 'Incomplete context — open Usage > Quality for details'
 ELSE 'Activity received; no detected context gaps' END AS "Collection status"
FROM coverage CROSS JOIN parameters p'''


def definitions():
    """Same parameter contract as explorer_activity; UTC half-open windows."""
    return [
        ('Activity compared with previous period', COMPARISON, 'table', {}, (35, 0, 24, 5)),
        ('Collection health', HEALTH, 'object', {}, (40, 0, 24, 7)),
        ('Collection status', STATUS, 'object', {}, (47, 0, 24, 3)),
    ]
