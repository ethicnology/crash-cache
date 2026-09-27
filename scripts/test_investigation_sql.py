"""Read-only contracts for the exact deployed investigation views.

Print PostgreSQL fixtures with --sql; execute with psql -v ON_ERROR_STOP=1.
No production rows, schema changes, writes or credentials are required.
"""
import json
from pathlib import Path
import re
import sys
import unittest

SQL = Path(__file__).resolve().parents[1] / 'docs/sql/investigation.sql'


def definitions():
    return re.findall(r'CREATE OR REPLACE VIEW crash_cache_explorer\.(\w+) AS\n(.*?);', SQL.read_text(), re.S)


def fixtures():
    reports = [
        dict(id=1, project_id=1, issue_id=7, event_id='shared', timestamp=100, user_id=1, exception_message_id=1, stacktrace_id=1, environment_id=1, app_version_id=1),
        dict(id=2, project_id=1, issue_id=7, event_id='second', timestamp=200, user_id=1, exception_message_id=2, environment_id=2, app_version_id=2),
        dict(id=3, project_id=2, issue_id=7, event_id='shared', timestamp=50, user_id=2, exception_message_id=3),
        dict(id=4, project_id=1, issue_id=None, event_id='activity', timestamp=300),
        dict(id=5, project_id=1, issue_id=8, event_id='anonymous', timestamp=400),
    ]
    columns = 'id integer,project_id integer,issue_id integer,event_id text,timestamp bigint,received_at timestamp,platform_id integer,environment_id integer,app_version_id integer,app_build_id integer,app_name_id integer,os_name_id integer,os_version_id integer,model_id integer,user_id integer,exception_type_id integer,exception_message_id integer,stacktrace_id integer,session_id integer'
    result = [f"report AS (SELECT * FROM jsonb_to_recordset('{json.dumps(reports)}'::jsonb) AS r({columns}))"]
    result += ["project(id,name,created_at) AS (VALUES (1,'First',timestamp '2026-01-01'),(2,'Second',timestamp '2026-01-01'))"]
    empty_lookups = ['platform','app_build','app_name','os_name','os_version','model','exception_type','tag_key','tag_value','breadcrumb_category','breadcrumb_type','breadcrumb_level']
    for name in empty_lookups:
        result.append(f'unwrap_{name}(id,value) AS (SELECT NULL::int,NULL::text WHERE false)')
    result += [
        "unwrap_environment(id,value) AS (VALUES (1,'production'),(2,'staging'))",
        "unwrap_app_version(id,value) AS (VALUES (1,'1.0'),(2,'2.0'))",
        "unwrap_user(id,value) AS (VALUES (1,'same-person'),(2,'other-person'))",
        "unwrap_exception_message(id,value) AS (VALUES (1,'Original'),(2,'Latest'),(3,'Other project secret'))",
        '''unwrap_stacktrace(id,frames) AS (VALUES (1,'[{"function":"outer","context_line":"call()","vars":{"x":1}},{"function":"inner"}]'::jsonb))''',
        'report_tag(report_id,key_id,value_id) AS (SELECT NULL::int,NULL::int,NULL::int WHERE false)',
        "unwrap_context_key(id,value) AS (VALUES (1,'trace'))",
        '''unwrap_context_value(id,value) AS (VALUES (1,'{"trace_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}'))''',
        'report_context(report_id,key_id,value_id) AS (VALUES (1,1,1),(3,1,1))',
        'report_breadcrumb(report_id,seq,breadcrumb_id) AS (VALUES (1,0,1),(1,1,2),(1,2,3),(3,0,1))',
        '''unwrap_breadcrumb(id,timestamp,category_id,type_id,level_id,data) AS (VALUES (1,2000::bigint,NULL::int,NULL::int,NULL::int,'{"message":"later"}'::jsonb),(2,1000,NULL,NULL,NULL,'{"message":"earlier"}'::jsonb),(3,NULL,NULL,NULL,NULL,'{"message":"unknown time"}'::jsonb))''',
        "attachment_metadata(id,project_id,event_id,filename,attachment_type,content_type,size_bytes,created_at) AS (VALUES (1::bigint,1,'shared','first.log','event.attachment','text/plain',10::bigint,timestamp '2026-01-01'),(2,2,'shared','second.log','event.attachment','text/plain',20,timestamp '2026-01-01'),(3,1,NULL,'unlinked.log','event.attachment','text/plain',5,timestamp '2026-01-01'))",
        "telemetry_log(id,project_id,timestamp,trace_id,span_id,level,body,severity_number,attributes) AS (VALUES (1::bigint,1,100::double precision,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',NULL::text,'error','first',17,'{}'::jsonb),(2,2,100,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',NULL,'error','second',17,'{}'::jsonb),(3,1,100,NULL,NULL,'info','unlinked',9,'{}'::jsonb))",
        "session(id,project_id,sid,distinct_id,started_at,timestamp,status_id,release_id,environment_id,errors,duration,abnormal_mechanism) AS (VALUES (1,1,'session-1','person-1','2026-09-01T00:00:00Z','2026-09-01T00:01:00Z',1,1,1,0,60::double precision,NULL::text))",
        "unwrap_session_status(id,value) AS (VALUES (1,'exited'))",
        "unwrap_session_release(id,value) AS (VALUES (1,'release-1'))",
        "unwrap_session_environment(id,value) AS (VALUES (1,'production'))",
    ]
    return result


CONTRACTS = [
    ('project_issue_isolation', "SELECT title,event_count,affected_identities FROM explorer_issues WHERE project_id=1 AND source_issue_id=7", [{'title':'Latest','event_count':2,'affected_identities':1}]),
    ('distinct_project_keys', 'SELECT count(DISTINCT id) AS n FROM explorer_issues WHERE source_issue_id=7', [{'n':2}]),
    ('project_first_last', 'SELECT extract(epoch FROM first_seen)::int AS first,extract(epoch FROM last_seen)::int AS last FROM explorer_issues WHERE project_id=1 AND source_issue_id=7', [{'first':100,'last':200}]),
    ('filtered_counts', "SELECT count(*) AS n FROM explorer_reports WHERE project_id=1 AND issue_key IS NOT NULL AND environment='production' AND app_version='1.0' AND event_at>=to_timestamp(100) AND event_at<to_timestamp(200)", [{'n':1}]),
    ('anonymous_visible', 'SELECT affected_identities,reports_without_identity FROM explorer_issues WHERE source_issue_id=8', [{'affected_identities':0,'reports_without_identity':1}]),
    ('frame_order_and_variables', "SELECT array_agg(function ORDER BY position) AS functions,max(variables->>'x') AS x,max(source_line) AS source FROM explorer_frames WHERE report_id=1", [{'functions':['outer','inner'],'x':'1','source':'call()'}]),
    ('breadcrumb_chronology', 'SELECT array_agg(message ORDER BY chronology) AS messages,min(extract(epoch FROM event_at))::int AS seconds FROM explorer_breadcrumbs WHERE report_id=1', [{'messages':['earlier','later','unknown time'],'seconds':1}]),
    ('breadcrumb_unique_keys', 'SELECT count(*)=count(DISTINCT id) AS unique_keys FROM explorer_breadcrumbs', [{'unique_keys':True}]),
    ('attachment_project_match', 'SELECT id,report_id FROM explorer_attachments ORDER BY id', [{'id':1,'report_id':1},{'id':2,'report_id':3},{'id':3,'report_id':None}]),
    ('log_trace_project_match', 'SELECT project_id,report_id,log_id FROM explorer_report_logs ORDER BY project_id', [{'project_id':1,'report_id':1,'log_id':1},{'project_id':2,'report_id':3,'log_id':2}]),
    ('uncorrelated_logs_retained', 'SELECT count(*) AS n FROM explorer_logs', [{'n':3}]),
    ('session_times', 'SELECT extract(epoch FROM updated_at-started_at)::int AS duration_seconds,release FROM explorer_sessions', [{'duration_seconds':60,'release':'release-1'}]),
]


def counting_sql():
    ctes = fixtures()
    for name, query in definitions():
        query = query.replace('public.', '').replace('crash_cache_explorer.', 'explorer_')
        ctes.append(f'explorer_{name} AS ({query})')
    print("BEGIN READ ONLY;\nSET LOCAL TIME ZONE 'Pacific/Honolulu';")
    for name, query, expected in CONTRACTS:
        expected_json = json.dumps(expected).replace("'", "''")
        print('WITH ' + ',\n'.join(ctes) + f", actual AS ({query})\nSELECT '{name}' AS contract, 1 / ((coalesce(jsonb_agg(to_jsonb(actual)), '[]'::jsonb) = '{expected_json}'::jsonb)::int) AS passed FROM actual;")
    print('ROLLBACK;')


class InvestigationContracts(unittest.TestCase):
    def test_complete_view_set(self):
        self.assertEqual([name for name, _ in definitions()], ['calendar','projects','reports','issues','frames','breadcrumbs','attachments','logs','report_logs','sessions'])

    def test_no_archive_or_dsn_read(self):
        for _, query in definitions():
            self.assertNotIn('public.archive', query)
            self.assertNotIn('public_key', query)
            self.assertNotIn('compressed_payload', query)

    def test_issue_aggregates_use_project_reports(self):
        query = dict(definitions())['issues']
        self.assertIn('GROUP BY issue_key,project_id,source_issue_id', query)
        self.assertNotIn('public.issue', query)


if __name__ == '__main__':
    if sys.argv[1:] == ['--sql']:
        counting_sql()
    else:
        unittest.main()
