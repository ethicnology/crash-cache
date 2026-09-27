"""Audience query contracts, plus read-only PostgreSQL fixtures for integration."""
import unittest

from explorer_activity import definitions


FIXTURES = '''reports(id,project_id,event_at,identity,os_name,platform,tags) AS (
 VALUES
 (1,1,'2026-09-01T00:00:00Z'::timestamptz,'alice','Android','java','{"event_kind":"app_activity"}'::jsonb),
 (2,1,'2026-09-05T12:00:00Z'::timestamptz,'alice','Android','java','{"event_kind":"app_session","app_session_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}'::jsonb),
 (3,1,'2026-09-05T13:00:00Z'::timestamptz,'alice','Android','java','{"event_kind":"app_activity","app_session_id":"aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"}'::jsonb),
 (4,1,'2026-09-06T13:00:00Z'::timestamptz,'alice','Android','java','{"event_kind":"app_session","app_session_id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}'::jsonb),
 (5,1,'2026-09-06T13:00:00Z'::timestamptz,'bob','Linux','native','{"event_kind":"app_activity"}'::jsonb),
 (6,2,'2026-09-06T13:00:00Z'::timestamptz,'bob','Windows','native','{"event_kind":"app_session","app_session_id":"cccccccccccccccccccccccccccccccc"}'::jsonb),
 (7,1,'2026-09-06T13:00:00Z'::timestamptz,'crash-only','Windows','native','{"event_kind":"exception"}'::jsonb),
 (8,1,'2026-09-07T00:00:00Z'::timestamptz,'excluded','Linux','native','{"event_kind":"app_activity"}'::jsonb)
), sessions(id,project_id,sid,identity,started_at,status) AS (
 VALUES
 (1,1,'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa','alice','2026-09-05T12:00:00Z'::timestamptz,'exited'),
 (2,1,'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb','alice','2026-09-06T13:00:00Z'::timestamptz,'ok'),
 (3,1,'cccccccc-cccc-cccc-cccc-cccccccccccc','bob','2026-09-06T13:00:00Z'::timestamptz,'exited'),
 (4,2,'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa','alice','2026-09-05T12:00:00Z'::timestamptz,'ok')
), '''


def fixture_query(name, grain='day', fixture=FIXTURES):
    query = next(sql for title, sql, _, _, _ in definitions() if title == name)
    for key, value in {'project_id': '1', 'from': "'2026-09-05T00:00:00Z'",
                       'until': "'2026-09-07T00:00:00Z'", 'grain': "'" + grain + "'"}.items():
        query = query.replace('{{' + key + '}}', value)
    return query.replace('WITH parameters', 'WITH ' + fixture + 'parameters', 1).replace(
        'crash_cache_explorer.reports', 'reports').replace('crash_cache_explorer.sessions', 'sessions')


def sql_contracts():
    """Return named read-only SELECT statements; each must return true on Postgres."""
    expected = {
        'Active installations in period': 'SELECT 2::bigint',
        'First observed installations in period': 'SELECT 1::bigint',
        'Sessions started in period': 'SELECT 3::bigint',
        'Started sessions still open': 'SELECT 1::bigint',
        'Active installations over time': "VALUES ('2026-09-05'::timestamp,1::bigint),('2026-09-06'::timestamp,2::bigint)",
        'Active installations by system': "VALUES ('Android',1::bigint),('Linux',1::bigint)",
        'Sessions started by system': "VALUES ('Android',2::bigint),('Unknown',1::bigint)",
        'Activity collection coverage': 'SELECT 4::bigint,0::bigint,0::bigint,1::bigint,0::bigint,1::bigint',
    }
    contracts = []
    for name, expected_sql in expected.items():
        query = fixture_query(name)
        contracts.append((name, f'''WITH actual AS ({query}), expected AS ({expected_sql})
SELECT NOT EXISTS ((SELECT * FROM actual EXCEPT ALL SELECT * FROM expected)
UNION ALL (SELECT * FROM expected EXCEPT ALL SELECT * FROM actual)) AS passed'''))
    weekly = fixture_query('Active installations over time', 'week')
    contracts.append(('Weekly distinct identity is not daily sum', f'''SELECT count(*)=1 AND max(active_installations)=2 FROM ({weekly}) q'''))
    invalid = fixture_query('Active installations in period', 'invalid')
    contracts.append(('Unsupported grain does not become SQL', f'''SELECT active_installations=0 FROM ({invalid}) q'''))
    # A different identity with the same session UUID must never assign a platform.
    other_identity = FIXTURES.replace(
        "(5,1,'2026-09-06T13:00:00Z'::timestamptz,'bob','Linux','native','{\"event_kind\":\"app_activity\"}'::jsonb)",
        "(5,1,'2026-09-06T13:00:00Z'::timestamptz,'other','Linux','native','{\"event_kind\":\"app_activity\",\"app_session_id\":\"cccccccccccccccccccccccccccccccc\"}'::jsonb)")
    isolated = fixture_query('Sessions started by system', fixture=other_identity)
    contracts.append(('Identity isolation', f"SELECT count(*)=1 FROM ({isolated}) q WHERE system='Unknown' AND sessions_started=1"))
    return contracts


class ActivityQueryTests(unittest.TestCase):
    def test_unique_panels_and_complete_parameters(self):
        panels = definitions()
        self.assertEqual(len({name for name, *_ in panels}), len(panels))
        for _, sql, _, _, _ in panels:
            for parameter in ('project_id', 'from', 'until', 'grain'):
                self.assertIn('{{' + parameter + '}}', sql)
            if 'date_trunc' in sql:
                self.assertIn("AT TIME ZONE 'UTC'", sql)

    def test_fixture_contracts_are_read_only_and_fully_bound(self):
        for _, sql in sql_contracts():
            self.assertNotIn('{{', sql)
            self.assertNotIn('crash_cache_explorer.', sql)
            self.assertNotIn('CREATE ', sql)
            self.assertNotIn('INSERT ', sql)

    def test_correlation_requires_project_identity_and_session(self):
        for _, sql, _, _, _ in definitions():
            self.assertIn('o.project_id=s.project_id', sql)
            self.assertIn("o.identity=nullif(s.identity,'')", sql)
            self.assertIn("o.canonical_sid=replace(lower(s.sid),'-','')", sql)


if __name__ == '__main__':
    unittest.main()
