"""Scope and repeatability contracts for app-usage provisioning."""
import unittest

from provision_activity import provision
from provision_metabase import ProvisionError
from test_provision_explorer import FakeApi


class ActivityApi(FakeApi):
    def call(self, method, path, body=None):
        if path.startswith('/card/') and path.endswith('/query'):
            self.calls.append((method,path,body))
            return {'status': 'completed', 'data': {'rows': []}}
        return super().call(method,path,body)


class ActivityProvisionTests(unittest.TestCase):
    def test_repeatable_and_explicit_utc_half_open_filters(self):
        api=ActivityApi()
        first=provision(api,2,6,103,'2026-09-06','2026-09-26')
        second=provision(api,2,6,103,'2026-09-06','2026-09-26')
        self.assertEqual(first,second)
        self.assertEqual(len(api.cards),19)
        self.assertEqual(len(api.dashboards),1)
        filters={p['id']:p for p in api.dashboards[0]['parameters']}
        self.assertEqual(filters['period']['default'],'2026-09-06~2026-09-26')
        self.assertEqual(filters['grain']['values_source_config']['values'],[['hour','Hour'],['day','Day'],['week','Week'],['month','Month']])
        for tile in api.dashboards[0]['dashcards']:
            self.assertEqual({p['parameter_id'] for p in tile['parameter_mappings']},{'project_id','period','grain'})
        self.assertFalse(any('public_link' in path for _,path,_ in api.calls))

    def test_invalid_scope_refused_before_changes(self):
        for project,start,end,grain in ((0,'2026-09-06','2026-09-26','day'),(103,'2026-09-26','2026-09-06','day'),(103,'2026-09-06','2026-09-26','quarter')):
            api=ActivityApi()
            with self.assertRaises(ProvisionError):
                provision(api,2,6,project,start,end,grain)
            self.assertEqual(api.calls,[])

    def test_public_question_refused(self):
        api=ActivityApi()
        provision(api,2,6,103,'2026-09-06','2026-09-26')
        api.cards[0]['public_uuid']='shared'
        with self.assertRaises(ProvisionError):
            provision(api,2,6,103,'2026-09-06','2026-09-26')


if __name__=='__main__':
    unittest.main()
