"""Date filters keep selected boundaries independent of retained observations."""
import unittest
from explorer_filters import period_sql, query_parameters, tags
from provision_metabase import ProvisionError


def sql_contracts():
    template="""WITH parameters AS (
 SELECT ({{from}}::date::timestamp AT TIME ZONE 'UTC') AS since,
 ({{until}}::date::timestamp AT TIME ZONE 'UTC') AS until, 'day'::text AS grain
) SELECT * FROM parameters"""
    dates="SELECT date '2024-02-28'+n AS day FROM generate_series(0,3) n"
    cases=[
        ("day BETWEEN date '2024-02-29' AND date '2024-03-01'", "since='2024-02-29T00:00:00Z'::timestamptz AND until='2024-03-02T00:00:00Z'::timestamptz"),
        ("day=date '2024-02-29'", "extract(epoch FROM until)-extract(epoch FROM since)=86400"),
        ("day IN (date '2024-02-28',date '2024-03-01')", 'since IS NULL AND until IS NULL'),
        ('false','since IS NULL AND until IS NULL'),
    ]
    output=[]
    for predicate,expected in cases:
        sql=period_sql(template).replace('{{period}}',predicate).replace('crash_cache_explorer.calendar','calendar')
        sql=sql.replace('WITH selected_days','WITH calendar AS ('+dates+'), selected_days',1)
        output.append('SELECT '+expected+' FROM ('+sql+') q')
    scalar_template=template.replace('SELECT * FROM parameters','SELECT count(*) FROM parameters WHERE since<until')
    invalid=period_sql(scalar_template,scalar=True).replace('{{period}}','false').replace('crash_cache_explorer.calendar','calendar')
    invalid=invalid.replace('WITH selected_days','WITH calendar AS ('+dates+'), selected_days',1)
    output.append('SELECT count(*)=0 FROM ('+invalid+') q')
    return output


class FilterTests(unittest.TestCase):
    def test_contiguous_calendar_bounds_and_field_targets(self):
        self.assertEqual(len(sql_contracts()),5)
        for sql in sql_contracts():self.assertNotIn('{{',sql)
        period=next(p for p in query_parameters(103,'past7days') if p['target'][1][1]=='period')
        self.assertEqual(period['target'],['dimension',['template-tag','period']])
        self.assertEqual(period['value'],'past7days')

    def test_query_parameter_ids_match_native_tag_ids(self):
        configured=tags({'day_field':591},103,'past7days')
        for p in query_parameters(105,'2026-09-26'):
            self.assertEqual(p['id'],configured[p['target'][1][1]]['id'])

    def test_unrecognized_query_is_not_silently_wrapped(self):
        with self.assertRaises(ProvisionError):period_sql('SELECT 1')


if __name__=='__main__':unittest.main()
