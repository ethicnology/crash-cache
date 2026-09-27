"""Offline parameter checks and SELECT-only PostgreSQL counting regressions.

Emit the SQL with --sql; run it using psql -v ON_ERROR_STOP=1 on a read-only
connection. Fixtures shadow real tables and never create or modify objects.
"""

import json
import sys
import unittest

from dashboard_charts import GRAINS, specifications
from provision_metabase import load_queries, query_parameters, template_tags


FIXTURES = """report(id, project_id, timestamp, user_id, os_name_id, model_id, app_version_id) AS (
    VALUES (1, 1, extract(epoch FROM timestamptz '2026-09-26 15:00Z'), 1, 1, 1, 1),
           (2, 1, extract(epoch FROM timestamptz '2026-09-27 15:00Z'), 1, 1, 1, 2),
           (3, 1, extract(epoch FROM timestamptz '2026-09-27 15:00Z'), 2, 1, 1, 2),
           (4, 1, extract(epoch FROM timestamptz '2026-09-28 15:00Z'), 1, 1, 1, 2),
           (5, 1, extract(epoch FROM timestamptz '2026-10-01 15:00Z'), 1, 1, 1, 2),
           (6, 2, extract(epoch FROM timestamptz '2026-09-27 15:00Z'), 3, 1, 1, 2),
           (7, 1, extract(epoch FROM timestamptz '2026-10-02 00:00Z'), 3, 1, 1, 2),
           (8, 1, extract(epoch FROM timestamptz '2026-09-27 15:00Z'), NULL, 1, 1, 2),
           (9, 1, extract(epoch FROM timestamptz '2026-09-27 15:00Z'), 3, 1, 1, 2)
), unwrap_user(id,value) AS (VALUES (1,'installation-a'),(2,'installation-b'),(3,'excluded')),
unwrap_tag_key(id,value) AS (VALUES (1,'event_kind'),(2,'app_session_id')),
unwrap_tag_value(id,value) AS (VALUES (1,'app_activity'),(2,'app_session'),(3,'error')),
report_tag(report_id,key_id,value_id) AS (
    SELECT id,1,CASE WHEN id=9 THEN 3 WHEN id=1 THEN 2 ELSE 1 END FROM report
), unwrap_os_name(id,value) AS (VALUES (1,'Android')),
unwrap_model(id,value) AS (VALUES (1,'Test phone')),
session(sid,distinct_id,started_at,project_id) AS (
    SELECT id::text, user_id::text, to_timestamp(timestamp)::text, project_id FROM report WHERE id<=7
)"""


def counting_sql():
    definitions = specifications(load_queries())
    cases = [
        ("whole_range_not_sum_of_days", definitions[0]["query"], [{"active_installations": 2}]),
        ("device_not_sum_of_versions", definitions[7]["query"], [{"device": "Test phone / Android", "device_model": "Test phone", "observed_installations": 2}]),
    ]
    expected_periods = {
        "hour": [("2026-09-26 15:00 UTC", 1), ("2026-09-27 15:00 UTC", 2), ("2026-09-28 15:00 UTC", 1), ("2026-10-01 15:00 UTC", 1)],
        "day": [("2026-09-26 00:00 UTC", 1), ("2026-09-27 00:00 UTC", 2), ("2026-09-28 00:00 UTC", 1), ("2026-10-01 00:00 UTC", 1)],
        "week": [("2026-09-21 00:00 UTC", 2), ("2026-09-28 00:00 UTC", 1)],
        "month": [("2026-09-01 00:00 UTC", 2), ("2026-10-01 00:00 UTC", 1)],
    }
    for grain, values in expected_periods.items():
        cases.append(("activity_" + grain, definitions[3]["query"].replace("{{grain}}", "'" + grain + "'"),
                      [{"period_start": period, "observed_active_installations": count} for period, count in values]))
    cases.append(("sessions_remain_additive", definitions[4]["query"].replace("{{grain}}", "'month'"),
                  [{"period_start": "2026-09-01 00:00 UTC", "sessions_started": 4},
                   {"period_start": "2026-10-01 00:00 UTC", "sessions_started": 1}]))
    cases.append(("single_day_hour_labels", definitions[3]["query"].replace("{{grain}}", "'hour'").replace("{{until}}", "'2026-09-27T00:00:00Z'"),
                  [{"period_start": "15:00 UTC", "observed_active_installations": 1}]))
    cases.append(("invalid_grain_returns_no_rows", definitions[3]["query"].replace("{{grain}}", "'invalid'"), []))
    print("BEGIN READ ONLY;\nSET LOCAL TIME ZONE 'Pacific/Honolulu';")
    for name, query, expected in cases:
        query = query.replace("{{project_id}}", "1").replace("{{from}}", "'2026-09-26T00:00:00Z'").replace("{{until}}", "'2026-10-02T00:00:00Z'")
        expected_json = json.dumps(expected).replace("'", "''")
        print(f"WITH {FIXTURES}, actual AS ({query})\nSELECT '{name}' AS contract,\n  1 / (coalesce(jsonb_agg(to_jsonb(actual)), '[]'::jsonb) = '{expected_json}'::jsonb)::int AS passed FROM actual;")
    print("ROLLBACK;")


class DashboardParameters(unittest.TestCase):
    def test_detail_tables_do_not_require_chart_grain(self):
        self.assertNotIn("grain", template_tags(1, "2026-09-26", "2026-09-27"))
        self.assertEqual(len(query_parameters(1, "2026-09-26", "2026-09-27")), 3)

    def test_grain_filter_uses_matching_native_parameter_identity(self):
        for grain in GRAINS:
            tags = template_tags(1, "2026-09-26", "2026-09-27", grain)
            parameter = query_parameters(1, "2026-09-26", "2026-09-27", grain)[-1]
            self.assertEqual(parameter["id"], tags["grain"]["id"])
            self.assertEqual(parameter["value"], grain)
            self.assertEqual(parameter["target"], ["variable", ["template-tag", "grain"]])


if __name__ == "__main__":
    if sys.argv[1:] == ["--sql"]:
        counting_sql()
    else:
        unittest.main()
