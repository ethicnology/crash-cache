"""Reprovisioning retains native tabs, including real API reset semantics."""
import copy
import unittest

from explorer_details import provision as details, link_events, link_issues
from explorer_workspace import organize
from provision_activity import provision as activity
from provision_explorer import provision
from test_provision_explorer import FakeApi


class TabApi(FakeApi):
    def __init__(self):
        super().__init__()
        self.next_tab = 100
        self.next_tile = 1000

    def call(self, method, path, body=None):
        if path.endswith('/query'):
            return {'status':'completed','data':{'rows':[],'cols':[]}}
        if method == 'PUT' and path.startswith('/dashboard/') and 'dashcards' in body:
            body = copy.deepcopy(body)
            # Metabase treats omitted tabs as an empty list, deleting old tabs.
            body['tabs'] = body.get('tabs') or []
            mapping = {}
            for tab in body['tabs']:
                if tab['id'] < 0:
                    mapping[tab['id']] = self.next_tab
                    tab['id'] = self.next_tab
                    self.next_tab += 1
            allowed = {tab['id'] for tab in body['tabs']}
            for tile in body['dashcards']:
                tab_id = tile.get('dashboard_tab_id')
                tile['dashboard_tab_id'] = mapping.get(tab_id, tab_id)
                if allowed and tile['dashboard_tab_id'] not in allowed:
                    raise AssertionError('Tabbed dashboards require a valid tab on every tile')
                if tile['id'] < 0:
                    tile['id'] = self.next_tile
                    self.next_tile += 1
        return super().call(method, path, body)


def provision_all(api):
    result = provision(api,7,'2026-09-01','2026-09-21')
    result['details'] = details(api,2,result['collection_id'],7,1,'',result['dashboards']['Health overview'])
    audience = activity(api,2,result['collection_id'],7,'2026-09-01','2026-09-21')
    for dashboard in (result['dashboards']['Health overview'],result['dashboards']['Event investigation']):
        link_issues(api,dashboard,result['cards']['issue_activity'],result['dashboards']['Event investigation'])
    link_events(api,result['dashboards']['Event investigation'],result['cards']['events'],result['details']['dashboard_id'])
    return result,audience


class TabPersistenceTests(unittest.TestCase):
    def test_full_reprovision_preserves_native_tab_ids_and_membership(self):
        api=TabApi()
        result,audience=provision_all(api)
        first=organize(api,result,audience)
        before={d['id']:{t['card_id']:t['dashboard_tab_id'] for t in d['dashcards'] if t.get('card_id')}
                for d in api.dashboards}
        result,audience=provision_all(api)
        interim={d['id']:{t['card_id']:t['dashboard_tab_id'] for t in d['dashcards'] if t.get('card_id')}
                 for d in api.dashboards}
        self.assertEqual(before,interim)
        self.assertEqual(first,organize(api,result,audience))

    def test_each_linker_preserves_existing_tab_ids(self):
        api=TabApi()
        result,audience=provision_all(api)
        organize(api,result,audience)
        dashboard=result['dashboards']['Health overview']
        before=copy.deepcopy(api.call('GET',f'/dashboard/{dashboard}'))
        for linker in (link_issues,link_events):
            with self.subTest(linker=linker.__name__):
                linker(api,dashboard,result['cards']['issue_activity'],result['dashboards']['Event investigation'])
                after=api.call('GET',f'/dashboard/{dashboard}')
                self.assertEqual(before['tabs'],after['tabs'])
                self.assertEqual([t['dashboard_tab_id'] for t in before['dashcards']],
                                 [t['dashboard_tab_id'] for t in after['dashcards']])


if __name__=='__main__':
    unittest.main()
