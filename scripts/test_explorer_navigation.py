"""Navigation preserves filter scope and per-tab placement on repeated runs."""
import copy
import unittest
from explorer_navigation import provision
from explorer_workspace import organize
from provision_explorer_home import provision as home
from test_explorer_tab_persistence import TabApi, provision_all


class NavigationTests(unittest.TestCase):
    def test_summary_figures_precede_navigation(self):
        api=TabApi()
        result,audience=provision_all(api)
        result['activity']=audience
        result['home']=home(api,result)
        organize(api,result,audience)
        nav=provision(api,result)
        dashboard=api.call('GET',f"/dashboard/{result['home']['dashboard_id']}")
        nav_tile=next(t for t in dashboard['dashcards'] if t.get('card_id')==nav['card_id'])
        for card_id in (audience['cards']['Active installations in period'],result['cards']['count']):
            figure=next(t for t in dashboard['dashcards'] if t.get('card_id')==card_id)
            self.assertGreaterEqual(nav_tile['row'],figure['row']+figure['size_y'])

    def test_scope_links_and_tab_membership_survive_rerun(self):
        api=TabApi()
        result,audience=provision_all(api)
        result['activity']=audience
        result['home']=home(api,result)
        organize(api,result,audience)
        nav=provision(api,result)
        before=copy.deepcopy(api.dashboards)
        self.assertEqual(nav,provision(api,result))
        self.assertEqual(before,api.dashboards)
        for dashboard in api.dashboards:
            tiles=[t for t in dashboard['dashcards'] if t.get('card_id')==nav['card_id']]
            expected={t['id'] for t in dashboard.get('tabs',[])} or {None}
            self.assertEqual({t['dashboard_tab_id'] for t in tiles},expected)
            for tile in tiles:
                for setting in tile['visualization_settings']['column_settings'].values():
                    link=setting['click_behavior']['linkTemplate']
                    self.assertIn('project={{project}}&period={{period}}',link)
            self.assertTrue(any(p['slug']=='period' for p in dashboard['parameters']))


if __name__=='__main__':unittest.main()
