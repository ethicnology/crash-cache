"""Focused contracts for layout-only Explorer organization."""
import copy
import unittest

from explorer_workspace import layout
from provision_metabase import ProvisionError


class LayoutTests(unittest.TestCase):
    def setUp(self):
        self.current = {'tabs': [], 'dashcards': [
            {'id': 11, 'card_id': 101, 'parameter_mappings': [{'parameter_id': 'period'}],
             'visualization_settings': {'click_behavior': {'linkTemplate': '/dashboard/10?event={{id}}'}}, 'series': []},
            {'id': 12, 'card_id': 102, 'parameter_mappings': [], 'visualization_settings': {}, 'series': []},
        ]}
        self.sections = [('Overview', 'Read the overview.', [(101,0,0,24,7)]),
                         ('Details', 'Read evidence.', [(102,0,0,24,9)])]

    def test_atomic_temporary_tab_ids_match_every_tile(self):
        body = layout(self.current, self.sections, 'Navigation')
        ids = {tab['id'] for tab in body['tabs']}
        self.assertEqual(ids, {-1,-2})
        self.assertTrue(all(tile['dashboard_tab_id'] in ids for tile in body['dashcards']))
        self.assertEqual(len({tile['id'] for tile in body['dashcards']}), 4)

    def test_query_scope_and_click_links_survive_without_mutating_source(self):
        before = copy.deepcopy(self.current)
        body = layout(self.current, self.sections, 'Navigation')
        card = next(tile for tile in body['dashcards'] if tile['card_id'] == 101)
        self.assertEqual(card['parameter_mappings'], before['dashcards'][0]['parameter_mappings'])
        self.assertEqual(card['visualization_settings'], before['dashcards'][0]['visualization_settings'])
        self.assertEqual(card['row'], 3)
        self.assertEqual(self.current, before)
        self.assertNotIn('parameters', body)

    def test_rerun_reuses_persisted_tabs_and_note_ids(self):
        first = layout(self.current, self.sections, 'Navigation')
        for tab in first['tabs']:
            old = tab['id']
            tab['id'] = 100-old
            for tile in first['dashcards']:
                if tile['dashboard_tab_id'] == old:
                    tile['dashboard_tab_id'] = tab['id']
        for index, tile in enumerate(first['dashcards']):
            if tile['id'] < 0:
                tile['id'] = 900+index
        self.assertEqual(layout(first, self.sections, 'Navigation'), first)

    def test_renamed_tabs_retain_ids_and_header_tiles(self):
        aliases = {'Breadcrumbs & logs': 'Breadcrumbs', 'Attachments': 'Files',
                   'Devices & versions': 'Devices', 'Collection quality': 'Quality'}
        for old, new in aliases.items():
            with self.subTest(tab=old):
                current = copy.deepcopy(self.current)
                current['tabs'] = [{'id': 51, 'name': old}, {'id': 52, 'name': 'Other'}]
                current['dashcards'].append({'id': 99, 'card_id': None,
                                            'dashboard_tab_id': 51})
                sections = [(new, 'Details.', [(101,0,0,24,7)]),
                            ('Other', 'More.', [(102,0,0,24,9)])]
                body = layout(current, sections, 'Navigation')
                self.assertEqual(body['tabs'][0], {'id': 51, 'name': new})
                self.assertEqual(body['dashcards'][0]['id'], 99)
                self.assertEqual(body['dashcards'][1]['dashboard_tab_id'], 51)

    def test_omitted_duplicated_and_unknown_cards_are_rejected(self):
        for sections in ([self.sections[0]], self.sections+[self.sections[0]],
                         self.sections+[('Other','',[(999,0,0,24,4)])]):
            with self.assertRaises(ProvisionError):
                layout(self.current, sections, 'Navigation')

    def test_public_dashboard_is_rejected(self):
        self.current['public_uuid'] = 'shared'
        with self.assertRaises(ProvisionError):
            layout(self.current, self.sections, 'Navigation')


if __name__ == '__main__':
    unittest.main()
