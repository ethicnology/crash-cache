"""Pure builder and idempotence contracts; no Metabase connection."""
import unittest

from provision_explorer import (MARKER, TABLES, ProvisionError, definitions,
                               managed, mappings, provision)


class FakeApi:
    def __init__(self):
        self.session='already-authenticated'
        self.calls=[]
        self.collections=[]
        self.cards=[]
        self.dashboards=[]
        self.tables=[]
        fields=('id','project_id','issue_key','source_issue_id','event_at','identity','title','platform','environment','app_version','device_model','report_id','log_id','session_id','latest_report_id','started_at','status','release','errors','duration','level','body','trace_id','name')
        for i,name in enumerate(TABLES,1):
            self.tables.append({'id':i,'name':name,'schema':'crash_cache_explorer',
                                'fields':[{'id':i*100+j,'name':f} for j,f in enumerate(fields)]})

        self.tables.append({'id':10,'name':'calendar','schema':'crash_cache_explorer','fields':[{'id':1000,'name':'day'}]})

    def call(self,method,path,body=None):
        self.calls.append((method,path,body))
        if path=='/database/2/metadata':
            return {'tables':self.tables}
        if path.startswith(('/table/','/field/')):
            return body
        for singular,items in (('collection',self.collections),('card',self.cards),('dashboard',self.dashboards)):
            root='/'+singular
            if path==root:
                if method=='GET':
                    return items.copy()
                item={**body,'id':len(items)+1}
                items.append(item)
                return item.copy()
            if path.startswith(root+'/'):
                item=next(i for i in items if i['id']==int(path.split('/')[-1]))
                if method=='PUT':
                    item.update(body)
                return item.copy()
        raise AssertionError((method,path))


class ExplorerContracts(unittest.TestCase):
    def test_provision_is_idempotent_and_private(self):
        api=FakeApi()
        first=provision(api,7,'2026-09-01','2026-09-21')
        second=provision(api,7,'2026-09-01','2026-09-21')
        self.assertEqual(first,second)
        self.assertEqual(len(api.collections),1)
        self.assertEqual(len(api.cards),14)
        self.assertEqual(len(api.dashboards),4)
        self.assertFalse(any('public_link' in path for _,path,_ in api.calls))
        for card in api.cards:
            self.assertEqual(card['dataset_query']['type'],'query')
            self.assertNotIn('native',card['dataset_query'])

    def test_filters_map_to_rows_before_aggregation(self):
        api=FakeApi()
        provision(api,7)
        overview=api.dashboards[0]
        for dashcard in overview['dashcards']:
            if dashcard['card_id'] is None:
                continue
            self.assertEqual({m['parameter_id'] for m in dashcard['parameter_mappings']},
                             {'project','period','environment','version'})
            self.assertTrue(all(m['target'][0]=='dimension' for m in dashcard['parameter_mappings']))
        issue=next(c for c in api.cards if c['name']=='Problems to investigate')
        self.assertEqual(issue['dataset_query']['query']['source-table'],3)
        self.assertEqual(issue['dataset_query']['query']['aggregation'][0],['count'])

    def test_composite_log_relation_does_not_invent_pk(self):
        api=FakeApi()
        # Real relation contains no singular id; remove the fixture convenience ID.
        relation=next(t for t in api.tables if t['name']=='report_logs')
        removed=next(f['id'] for f in relation['fields'] if f['name']=='id')
        relation['fields']=[f for f in relation['fields'] if f['name']!='id']
        provision(api,7)
        self.assertFalse(any(path==f'/field/{removed}' for _,path,_ in api.calls))
        log_id=next(f['id'] for f in relation['fields'] if f['name']=='log_id')
        update=next(body for _,path,body in api.calls if path==f'/field/{log_id}')
        self.assertEqual(update['semantic_type'],'type/FK')

    def test_unmanaged_collision_refused(self):
        with self.assertRaises(ProvisionError):
            managed([{'name':'Existing','description':'User authored'}],'Existing')
        with self.assertRaises(ProvisionError):
            managed([{'name':'Existing','description':MARKER}]*2,'Existing')

    def test_shared_objects_refused(self):
        for object_name in ('card','dashboard'):
            with self.assertRaises(ProvisionError):
                managed([{'name':object_name,'description':MARKER,'public_uuid':'shared'}],object_name)

    def test_issue_and_release_filters_and_navigation(self):
        api=FakeApi()
        provision(api,7)
        sessions=next(d for d in api.dashboards if d['name']=='Sessions and logs')
        self.assertIn('release',[p['id'] for p in sessions['parameters']])
        self.assertNotIn('version',[p['id'] for p in sessions['parameters']])
        investigation=next(d for d in api.dashboards if d['name']=='Event investigation')
        self.assertIn('issue',[p['id'] for p in investigation['parameters']])
        for card in investigation['dashcards']:
            if card['card_id']:
                self.assertIn('issue',[m['parameter_id'] for m in card['parameter_mappings']])
        releases=next(d for d in api.dashboards if d['name']=='Releases and devices')
        self.assertEqual({p['id'] for p in releases['parameters']},{'project','period','environment','version','release'})
        for dashcard in releases['dashcards']:
            if dashcard['card_id']:
                card=next(c for c in api.cards if c['id']==dashcard['card_id'])
                actual={m['parameter_id'] for m in dashcard['parameter_mappings']}
                is_sessions=card['name']=='Session outcomes by release'
                self.assertEqual(actual,{'project','period','environment','release' if is_sessions else 'version'})
        device=next(c for c in api.cards if c['name']=='Most affected device models')
        self.assertEqual(device['dataset_query']['query']['limit'],10)
        for dashboard in api.dashboards:
            note=dashboard['dashcards'][0]
            self.assertIsNone(note['card_id'])
            self.assertEqual(note['visualization_settings']['text'].count('](/dashboard/'),4)
            self.assertIn('default filters',note['visualization_settings']['text'])

    def test_error_investigation_excludes_activity_events(self):
        api=FakeApi()
        provision(api,7)
        events=next(c for c in api.cards if c['name']=='Event records')
        query=events['dataset_query']['query']
        issue_field=next(f['id'] for t in api.tables if t['name']=='reports' for f in t['fields'] if f['name']=='issue_key')
        self.assertEqual(query.get('filter'),['not-null',['field',issue_field,None]])

    def test_missing_schema_refused_before_mutation(self):
        api=FakeApi()
        api.tables=[]
        with self.assertRaises(ProvisionError):
            provision(api,7)
        self.assertEqual(len(api.calls),1)

    def test_invalid_date_range_refused(self):
        api=FakeApi()
        with self.assertRaises(ProvisionError):
            provision(api,7,'2026-09-21','2026-09-01')
        self.assertEqual(api.calls,[])


if __name__=='__main__':
    unittest.main()
