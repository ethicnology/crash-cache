"""Scoped detail-card contracts and read-only PostgreSQL fixtures (--sql)."""

import json
import sys
import unittest

from detail_charts import FILTERS, specifications


FIXTURES = """report(id,project_id,timestamp,issue_id,stacktrace_id,platform_id,app_version_id,model_id,exception_type_id) AS (
 VALUES (1,1,extract(epoch FROM timestamptz '2026-09-26 01:00Z'),1,1,1,1,1,1),
 (2,1,extract(epoch FROM timestamptz '2026-09-26 02:00Z'),2,NULL,1,2,1,2),
 (3,1,extract(epoch FROM timestamptz '2026-09-26 03:00Z'),3,2,2,1,2,3),
 (4,1,extract(epoch FROM timestamptz '2026-09-26 04:00Z'),4,NULL,NULL,NULL,NULL,NULL),
 (5,2,extract(epoch FROM timestamptz '2026-09-26 01:00Z'),1,1,1,1,1,1),
 (6,1,extract(epoch FROM timestamptz '2026-09-27 00:00Z'),1,1,1,1,1,1),
 (7,1,extract(epoch FROM timestamptz '2026-09-25 23:59Z'),1,1,1,1,1,1),
 (8,1,extract(epoch FROM timestamptz '2026-09-26 01:00Z'),NULL,1,1,1,1,1)
), unwrap_platform(id,value) AS (VALUES (1,'native'),(2,'dart')),
unwrap_app_version(id,value) AS (VALUES (1,'1.0'),(2,'2.0')),
unwrap_model(id,value) AS (VALUES (1,'Test phone'),(2,'Other phone')),
unwrap_exception_type(id,value) AS (VALUES (1,'SIGSEGV'),(2,'Panic'),(3,'custom.category')),
unwrap_tag_key(id,value) AS (VALUES (1,'layer'),(2,'component'),(3,'irrelevant')),
unwrap_tag_value(id,value) AS (VALUES (1,'godot'),(2,'rust'),(3,'flutter'),(4,'runtime'),(5,'storage'),(6,'custom.category')),
report_tag(report_id,key_id,value_id) AS (
 VALUES (1,1,1),(1,2,4),(1,3,6),(2,1,2),(2,2,5),(3,1,3),(3,2,6),
 (5,1,1),(6,1,1),(7,1,1),(8,1,1)
), report_breadcrumb(report_id,seq) AS (VALUES (1,0),(1,1),(3,0)),
unwrap_stacktrace(id,frames) AS (VALUES (1,'[{"function":"private"}]'::jsonb),(2,'null'::jsonb))"""


SUMMARY_ROWS = [
 {"event_time_utc":"2026-09-26 04:00:00","layer":"Unclassified","component":"Unclassified","runtime":"Unknown runtime","app_version":"Unknown version","device_model":"Unknown device","exception_type":"Unclassified","has_breadcrumbs":False,"has_frames":False},
 {"event_time_utc":"2026-09-26 03:00:00","layer":"flutter","component":"custom.category","runtime":"dart","app_version":"1.0","device_model":"Other phone","exception_type":"custom.category","has_breadcrumbs":True,"has_frames":False},
 {"event_time_utc":"2026-09-26 02:00:00","layer":"rust","component":"storage","runtime":"native","app_version":"2.0","device_model":"Test phone","exception_type":"Panic","has_breadcrumbs":False,"has_frames":False},
 {"event_time_utc":"2026-09-26 01:00:00","layer":"godot","component":"runtime","runtime":"native","app_version":"1.0","device_model":"Test phone","exception_type":"SIGSEGV","has_breadcrumbs":True,"has_frames":True},
]


def counting_sql():
    specs = specifications()
    cases = [
        ("all_timeline", 0, {}, [{"day":"2026-09-26","reports":4}]),
        ("all_components", 1, {}, [{"component":c,"reports":1} for c in ("custom.category","Unclassified","runtime","storage")]),
        ("all_types", 2, {}, [{"exception_type":e,"reports":1} for e in ("custom.category","Panic","SIGSEGV","Unclassified")]),
        ("all_summaries", 3, {}, SUMMARY_ROWS),
        ("layer_drill", 3, {"layer":"godot"}, [SUMMARY_ROWS[3]]),
        ("version_drill", 0, {"app_version":"1.0"}, [{"day":"2026-09-26","reports":2}]),
        ("device_drill", 0, {"device_model":"Test phone"}, [{"day":"2026-09-26","reports":2}]),
        ("combined_drill", 3, {"layer":"rust","app_version":"2.0","device_model":"Test phone"}, [SUMMARY_ROWS[2]]),
        ("incompatible_filters", 0, {"layer":"rust","app_version":"1.0"}, []),
        ("missing_metadata_filter", 3, {"app_version":"Unknown version","device_model":"Unknown device","layer":"Unclassified"}, [SUMMARY_ROWS[0]]),
    ]
    print("BEGIN READ ONLY;\nSET LOCAL TIME ZONE 'Pacific/Honolulu';")
    for name, index, values, expected in cases:
        query = specs[index]["query"].replace("{{project_id}}","1").replace("{{from}}","'2026-09-26T00:00:00Z'").replace("{{until}}","'2026-09-27T00:00:00Z'")
        for key in FILTERS:
            query = query.replace("{{" + key + "}}", "'" + values.get(key, "All").replace("'", "''") + "'")
        expected_json = json.dumps(expected).replace("'", "''")
        print(f"WITH {FIXTURES}, actual AS ({query}), result AS (SELECT coalesce(jsonb_agg(to_jsonb(actual)), '[]'::jsonb) AS rows FROM actual)\nSELECT '{name}' AS contract, 1 / ((rows @> '{expected_json}'::jsonb) AND jsonb_array_length(rows)=jsonb_array_length('{expected_json}'::jsonb))::int AS passed FROM result;")
    print("ROLLBACK;")


class DetailContracts(unittest.TestCase):
    def test_every_card_accepts_same_optional_filters(self):
        for card in specifications():
            for parameter in ("project_id","from","until",*FILTERS):
                self.assertIn("{{" + parameter + "}}", card["query"])
            for parameter in FILTERS:
                self.assertIn("{{" + parameter + "}}='All'", card["query"])

    def test_summaries_exclude_raw_data_and_have_limit(self):
        query = specifications()[-1]["query"]
        for raw_field in ("event_id", "exception_message", "user_id", "vars", "context_line"):
            self.assertNotIn(raw_field, query)
        self.assertIn("LIMIT 200", query)
        self.assertIn("r.issue_id IS NOT NULL", query)

    def test_public_metadata_buckets_are_explicit(self):
        query = specifications()[0]["query"]
        self.assertIn("coalesce(e.value", query)
        self.assertIn("Unknown device", query)
        self.assertIn("Unknown version", query)
        self.assertNotIn("JOIN session", query)


if __name__ == "__main__":
    if sys.argv[1:] == ["--sql"]:
        counting_sql()
    else:
        unittest.main()
