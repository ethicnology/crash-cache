"""Pure summary contracts and executable, read-only PostgreSQL scenarios."""
import unittest

from explorer_summary import definitions


# Repeated identities are intentional. Other-project events must not contribute.
FIXTURE = '''reports(id,project_id,event_at,identity,os_name,platform,tags,issue_key,stack_frames) AS (
 VALUES
 (1,1,'2026-09-03T12:00:00Z'::timestamptz,'alice','Android','java','{"event_kind":"app_activity"}'::jsonb,NULL::bigint,NULL::jsonb),
 (2,1,'2026-09-05T12:00:00Z'::timestamptz,'alice','Android','java','{"event_kind":"app_activity"}'::jsonb,NULL,NULL),
 (3,1,'2026-09-06T12:00:00Z'::timestamptz,'alice','Android','java','{"event_kind":"app_activity"}'::jsonb,NULL,NULL),
 (4,1,'2026-09-06T12:00:00Z'::timestamptz,NULL,'Android','java','{"event_kind":"app_activity"}'::jsonb,NULL,NULL),
 (5,1,'2026-09-06T12:00:00Z'::timestamptz,'bob','Android','java','{}'::jsonb,1,NULL),
 (6,2,'2026-09-05T12:00:00Z'::timestamptz,'other','Linux','native','{"event_kind":"app_activity"}'::jsonb,2,'[]'::jsonb)
), sessions(id,project_id,sid,identity,started_at,status) AS (
 VALUES (1,1,'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa','alice','2026-09-05T12:00:00Z'::timestamptz,'ok')
), '''


def query(name, start='2026-09-05', end='2026-09-07', project=1):
    sql = next(sql for title, sql, _, _, _ in definitions() if title == name)
    for key, value in {'project_id': str(project), 'from': "'" + start + "'",
                       'until': "'" + end + "'", 'grain': "'day'"}.items():
        sql = sql.replace('{{' + key + '}}', value)
    return sql.replace('WITH parameters', 'WITH ' + FIXTURE + 'parameters', 1).replace(
        'crash_cache_explorer.reports', 'reports').replace('crash_cache_explorer.sessions', 'sessions')


def sql_contracts():
    """Every statement must return true; no tables, roles or services are changed."""
    comparison = 'Activity compared with previous period'
    checks = [
        ('Repeated identity distinct across entire period', query(comparison),
         '''count(*)=1 FROM actual WHERE "Metric"='Active installations' AND "Current"=1
 AND "Previous"=1 AND "Absolute change"=0 AND "Change percent"=0 AND "Interpretation"='Comparable' '''),
        ('New error observations have no fabricated percent', query(comparison),
         '''count(*)=1 FROM actual WHERE "Metric"='Error occurrences' AND "Current"=1
 AND "Previous"=0 AND "Change percent" IS NULL AND "Interpretation"='Newly observed' '''),
        ('Empty period comparison retains all metric rows', query(comparison, project=99),
         '''count(*)=3 AND bool_and("Current"=0 AND "Previous"=0 AND "Change percent" IS NULL
 AND "Interpretation"='No previous observations') FROM actual'''),
        ('Current zero versus previous activity is minus 100 percent', query(comparison, '2026-09-07', '2026-09-09'),
         '''count(*)=1 FROM actual WHERE "Metric"='Active installations' AND "Current"=0
 AND "Previous"=1 AND "Absolute change"=-1 AND "Change percent"=-100 AND "Interpretation"='Comparable' '''),
        ('Partial collection remains visible', query('Collection health'),
         '''count(*)=1 FROM actual WHERE "Valid period" AND "Activity observations"=3
 AND "Activity missing identity"=1 AND "Started sessions without known system"=1
 AND "Error occurrences"=1 AND "Errors without stack frames"=1'''),
        ('Empty collection does not imply zero players', query('Collection health', project=99),
         '''count(*)=1 FROM actual WHERE "Valid period" AND "Activity observations"=0
 AND "Activity coverage"='No activity observations received' '''),
        ('Invalid period is explicit', query('Collection health', '2026-09-07', '2026-09-05'),
         '''count(*)=1 FROM actual WHERE NOT "Valid period" AND "Activity coverage"='Invalid period selection' '''),
    ]
    return [(name, 'WITH actual AS (' + sql + ') SELECT ' + expectation)
            for name, sql, expectation in checks]


class SummaryTests(unittest.TestCase):
    def test_comparison_uses_selected_duration_and_safe_percentage(self):
        sql = definitions()[0][1]
        self.assertIn('2*extract(epoch FROM since)-extract(epoch FROM until)', sql)
        self.assertIn('nullif(previous_value,0)', sql)
        self.assertNotIn('min(event_at)', sql)
        self.assertNotIn('max(event_at)', sql)
        self.assertIn('count(DISTINCT identity)', sql)

    def test_fixtures_are_bound_read_only_queries(self):
        self.assertEqual(len(sql_contracts()), 7)
        for _, sql in sql_contracts():
            self.assertNotIn('{{', sql)
            self.assertNotIn('crash_cache_explorer.', sql)
            self.assertNotIn('INSERT ', sql)
            self.assertNotIn('CREATE ', sql)


if __name__ == '__main__':
    unittest.main()
