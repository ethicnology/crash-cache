"""Offline chart contracts and SELECT-only PostgreSQL fixtures (emit with --sql)."""

import json
import sys
import unittest

from diagnostic_charts import pipeline_specifications, specifications


FIXTURES = """raw_report(id,project_id,timestamp,issue_id,stacktrace_id) AS (
 VALUES (1,1,extract(epoch FROM timestamptz '2026-09-26 00:00Z'),1,1),
        (2,1,extract(epoch FROM timestamptz '2026-09-26 23:59Z'),2,2),
        (3,1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),3,NULL),
        (4,2,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),4,1),
        (5,1,extract(epoch FROM timestamptz '2026-09-27 00:00Z'),5,1),
        (6,1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),NULL,1),
        (7,1,extract(epoch FROM timestamptz '2026-09-25 23:59Z'),7,1),
        (8,1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),8,3)
), report AS (SELECT *, NULL::int AS platform_id FROM raw_report), unwrap_platform(id,value) AS (VALUES (1,'python')), unwrap_stacktrace(id,frames) AS (
 VALUES (1,'[{"function":"sample","context_line":"private source","vars":{"secret":"private value"}}, {"function":"sample"}]'::jsonb),
        (2,'[{"function":null,"context_line":"","pre_context":[],"vars":{}}]'::jsonb),
        (3,'null'::jsonb)
), unwrap_tag_key(id,value) AS (VALUES (1,'layer')),
unwrap_tag_value(id,value) AS (VALUES (1,'godot'),(2,'worker')),
report_tag(report_id,key_id,value_id) AS (VALUES (1,1,1),(2,1,1),(3,1,2)),
unwrap_breadcrumb_category(id,value) AS (VALUES (1,'navigation'),(2,'db.query')),
unwrap_breadcrumb(id,category_id) AS (VALUES (1,1),(2,1),(3,2),(4,NULL)),
report_breadcrumb(report_id,seq,breadcrumb_id) AS (
 VALUES (1,0,1),(1,1,2),(1,2,3),(2,0,1),(2,1,4),(4,0,1),(5,0,1),(6,0,1),(7,0,1)
), unwrap_context_key(id,value) AS (VALUES (1,'symbolication'),(2,'queue'),(3,'job')),
unwrap_context_value(id,value) AS (VALUES (1,'{"status":"partial","resolved_frames":1}'),(2,'{"private":"value"}')),
report_context(report_id,key_id,value_id) AS (
 VALUES (1,1,1),(1,2,2),(1,3,2),(2,2,2),(4,1,1),(5,1,1),(6,1,1),(7,1,1)
)"""


EXPECTED = [
    [{"error_reports": 4}],
    [{"layer": "worker", "with_breadcrumbs": 0, "without_breadcrumbs": 1},
     {"layer": "Unclassified", "with_breadcrumbs": 0, "without_breadcrumbs": 1},
     {"layer": "godot", "with_breadcrumbs": 2, "without_breadcrumbs": 0}],
    [{"category": "navigation", "reports_with_category": 2},
     {"category": "db.query", "reports_with_category": 1},
     {"category": "Unclassified", "reports_with_category": 1}],
    [{"evidence": "Named function", "present": 1, "absent": 3},
     {"evidence": "Source context", "present": 1, "absent": 3},
     {"evidence": "Stack frames", "present": 2, "absent": 2},
     {"evidence": "Variables", "present": 1, "absent": 3}],
    [{"status": "Not recorded", "reports": 3}, {"status": "partial", "reports": 1}],
    [{"context_family": "queue", "reports_with_context": 2},
     {"context_family": "job", "reports_with_context": 1},
     {"context_family": "symbolication", "reports_with_context": 1}],
]


def counting_sql():
    print("BEGIN READ ONLY;\nSET LOCAL TIME ZONE 'Pacific/Honolulu';")
    for index, (definition, expected) in enumerate(zip(specifications(), EXPECTED)):
        query = definition["query"].replace("{{project_id}}", "1").replace("{{from}}", "'2026-09-26T00:00:00Z'").replace("{{until}}", "'2026-09-27T00:00:00Z'")
        expected_json = json.dumps(expected).replace("'", "''")
        # JSON containment is order-independent here; equal length excludes extra rows.
        print(f"WITH {FIXTURES}, actual AS ({query}), result AS (SELECT coalesce(jsonb_agg(to_jsonb(actual)), '[]'::jsonb) AS rows FROM actual)\nSELECT 'diagnostic_card_{index}' AS contract, 1 / ((rows @> '{expected_json}'::jsonb) AND jsonb_array_length(rows) = jsonb_array_length('{expected_json}'::jsonb))::int AS passed FROM result;")
    print("ROLLBACK;")


class DiagnosticChartContracts(unittest.TestCase):
    def test_every_card_is_scoped_to_error_reports(self):
        for definition in specifications():
            self.assertIn("{{project_id}}", definition["query"])
            self.assertIn("{{from}}", definition["query"])
            self.assertIn("{{until}}", definition["query"])
            self.assertIn("r.issue_id IS NOT NULL", definition["query"])
            self.assertNotIn("{{grain}}", definition["query"])

    def test_specs_have_unique_names_and_independent_layout(self):
        definitions = specifications()
        self.assertEqual(len(definitions), len(EXPECTED))
        self.assertEqual(len({card["name"] for card in definitions}), len(definitions))
        for card in definitions:
            row, col, width, height = card["layout"]
            self.assertGreaterEqual(row, 0)
            self.assertGreaterEqual(col, 0)
            self.assertLessEqual(col + width, 24)
            self.assertGreater(height, 0)

    def test_no_attachment_or_log_storage_is_assumed(self):
        for card in specifications():
            self.assertNotIn("FROM attachment", card["query"])
            self.assertNotIn("FROM log", card["query"])
            self.assertNotIn("SELECT cv.value", card["query"].split("SELECT status,")[-1])


PIPELINE_FIXTURES = """archive(hash,project_id,created_at) AS (
 VALUES ('a',1,timestamp '2026-09-26 00:00:00'),('b',1,timestamp '2026-09-26 23:59:00'),
 ('c',2,timestamp '2026-09-26 12:00:00'),('d',1,timestamp '2026-09-27 00:00:00'),
 ('e',1,timestamp '2026-09-25 23:59:00'),('f',1,timestamp '2026-09-26 12:00:00')
), queue(archive_hash) AS (VALUES ('a'),('c'),('d'),('e')),
queue_error(archive_hash) AS (VALUES ('b'),('c'),('d'),('e')),
telemetry_log(project_id,timestamp,level) AS (
 VALUES (1,extract(epoch FROM timestamptz '2026-09-26 00:00Z'),'info'),
 (1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),'info'),
 (1,extract(epoch FROM timestamptz '2026-09-26 23:59Z'),'error'),
 (1,extract(epoch FROM timestamptz '2026-09-26 23:59Z'),'private arbitrary level'),
 (2,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),'info'),
 (1,extract(epoch FROM timestamptz '2026-09-27 00:00Z'),'info'),
 (1,extract(epoch FROM timestamptz '2026-09-25 23:59Z'),'info')
), attachment_metadata(project_id,archive_hash,event_id,attachment_type,size_bytes,filename,content_type) AS (
 VALUES (1,'a','event1','event.attachment',100,'opaque.bin','application/octet-stream'),
 (1,'a','event1','event.attachment',200,'screenshot.png','text/plain'),
 (1,'a','event1','event.view_hierarchy',50,NULL,NULL),(1,'b',NULL,NULL,10,NULL,NULL),
 (1,'b','event3','private arbitrary type',5,NULL,NULL),(2,'c','event2','event.attachment',999,NULL,NULL),
 (1,'d','event2','event.minidump',300,NULL,NULL),(1,'e','old','event.minidump',400,NULL,NULL),
 (2,'c','event4','event.attachment',999,NULL,NULL),
 (1,'a','event1','event.attachment',1024,'screenshot.png','image/png'),
 (1,'a','event1',NULL,2048,'screenshot.jpg','image/jpeg'),
 (1,'a','event1','event.attachment',512,'screenshot.jpeg','image/jpeg'),
 (1,'a','event1','event.attachment',4096,'godot.log','text/plain'),
 (1,'a','event1','event.minidump',8192,'screenshot.png','image/png'),
 (1,'a','event1','event.applecrashreport',10240,NULL,NULL)
), report(project_id,timestamp,issue_id,event_id) AS (
 VALUES (1,extract(epoch FROM timestamptz '2026-09-26 00:00Z'),1,'event1'),
 (1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),2,'event2'),
 (1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),3,'event3'),
 (1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),4,'event4'),
 (1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),5,'noattachment'),
 (2,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),2,'event2'),
 (1,extract(epoch FROM timestamptz '2026-09-27 00:00Z'),2,'event2'),
 (1,extract(epoch FROM timestamptz '2026-09-25 23:59Z'),2,'event2'),
 (1,extract(epoch FROM timestamptz '2026-09-26 12:00Z'),NULL,'event1')
)"""

PIPELINE_EXPECTED = [
 [{"archives":3}], [{"pending":1}], [{"failed":1}],
 [{"severity":"info","logs":2},{"severity":"error","logs":1},{"severity":"Other","logs":1}],
 [{"day":"2026-09-26","trace":0,"debug":0,"info":2,"warn":0,"error":1,"fatal":0,"other":1}],
 [{"attachment_type":"Attachment","attachments":2},{"attachment_type":"View hierarchy","attachments":1},
  {"attachment_type":"Unspecified","attachments":1},{"attachment_type":"Other","attachments":1},
  {"attachment_type":"Screenshot","attachments":3},{"attachment_type":"Application log","attachments":1},
  {"attachment_type":"Native crash dump","attachments":1},{"attachment_type":"Apple crash report","attachments":1}],
 [{"attachment_type":"Attachment","payload_kib":0.293},{"attachment_type":"View hierarchy","payload_kib":0.049},
  {"attachment_type":"Unspecified","payload_kib":0.010},{"attachment_type":"Other","payload_kib":0.005},
  {"attachment_type":"Screenshot","payload_kib":3.5},{"attachment_type":"Application log","payload_kib":4},
  {"attachment_type":"Native crash dump","payload_kib":8},{"attachment_type":"Apple crash report","payload_kib":10}],
 [{"coverage":"With metadata","reports":3},{"coverage":"Without metadata","reports":2}],
]


def pipeline_counting_sql():
    print("BEGIN READ ONLY;\nSET LOCAL TIME ZONE 'Pacific/Honolulu';")
    for index, (definition, expected) in enumerate(zip(pipeline_specifications(), PIPELINE_EXPECTED)):
        query = definition["query"].replace("{{project_id}}", "1").replace("{{from}}", "'2026-09-26T00:00:00Z'").replace("{{until}}", "'2026-09-27T00:00:00Z'")
        expected_json = json.dumps(expected).replace("'", "''")
        print(f"WITH {PIPELINE_FIXTURES}, actual AS ({query}), result AS (SELECT coalesce(jsonb_agg(to_jsonb(actual)), '[]'::jsonb) AS rows FROM actual)\nSELECT 'pipeline_card_{index}' AS contract, 1 / ((rows @> '{expected_json}'::jsonb) AND jsonb_array_length(rows) = jsonb_array_length('{expected_json}'::jsonb))::int AS passed FROM result;")
    print("ROLLBACK;")


class PipelineChartContracts(unittest.TestCase):
    def test_required_filters_and_matching_fixtures(self):
        definitions = pipeline_specifications()
        self.assertEqual(len(definitions), len(PIPELINE_EXPECTED))
        for card in definitions:
            for parameter in ("project_id", "from", "until"):
                self.assertIn("{{" + parameter + "}}", card["query"])
            self.assertNotIn("{{grain}}", card["query"])

    def test_archive_time_is_explicit_utc(self):
        for card in pipeline_specifications():
            if "FROM archive" in card["query"]:
                self.assertIn("a.created_at AT TIME ZONE 'UTC'", card["query"])

    def test_no_raw_diagnostic_content_selected(self):
        for card in pipeline_specifications():
            for forbidden in (".body", ".attributes", ".error", ".compressed_payload"):
                self.assertNotIn(forbidden, card["query"])

    def test_attachment_size_declares_binary_units_and_rename(self):
        card = pipeline_specifications()[6]
        self.assertEqual(card["settings"]["graph.metrics"], ["payload_kib"])
        self.assertEqual(card["previous_name"], "Received attachment bytes")
        self.assertIn("1 KiB = 1,024 bytes", card["description"])

    def test_attachment_filenames_are_only_used_for_classification(self):
        for card in pipeline_specifications()[5:7]:
            final_select = card["query"].rsplit(") SELECT ", 1)[-1]
            self.assertNotIn("filename", final_select)
            self.assertNotIn("content_type", final_select)


if __name__ == "__main__":
    if sys.argv[1:] == ["--sql"]:
        counting_sql()
        pipeline_counting_sql()
    elif sys.argv[1:] == ["--pipeline-sql"]:
        pipeline_counting_sql()
    else:
        unittest.main()
