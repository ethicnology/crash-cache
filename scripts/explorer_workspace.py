"""Arrange existing Explorer questions into native Metabase dashboard tabs.

Metabase v0.63.18 dashboards_rest/api.clj accepts temporary negative tab IDs in
PUT /dashboard/:id and rewrites matching dashboard_tab_id values atomically.
No question, query, filter default, permission or public link is created here.
"""
from copy import deepcopy
import os

from provision_metabase import ProvisionError

TAB_ALIASES = {
    'Breadcrumbs': ('Breadcrumbs & logs',),
    'Files': ('Attachments',),
    'Devices': ('Devices & versions',),
    'Quality': ('Collection quality',),
}


def _tile(source, tab_id, row, col, width, height):
    keys = ('id', 'card_id', 'series', 'parameter_mappings', 'visualization_settings',
            'inline_parameters', 'action_id')
    tile = {key: deepcopy(source[key]) for key in keys if key in source}
    tile.update(dashboard_tab_id=tab_id, row=row, col=col, size_x=width, size_y=height)
    return tile


def layout(current, sections, navigation):
    """Return one atomic layout update, preserving all question tiles exactly once."""
    if current.get('public_uuid'):
        raise ProvisionError('Explorer workspace must remain authenticated')
    cards = [tile for tile in current.get('dashcards', []) if tile.get('card_id')]
    by_card = {tile['card_id']: tile for tile in cards}
    requested = [entry[0] for _, _, entries in sections for entry in entries]
    if len(by_card) != len(cards) or len(requested) != len(set(requested)) or set(requested) != set(by_card):
        raise ProvisionError('Workspace layout must account for every question exactly once')
    old_tabs = {tab['name']: tab['id'] for tab in current.get('tabs', [])}
    old_notes = {tile.get('dashboard_tab_id'): tile for tile in current.get('dashcards', [])
                 if not tile.get('card_id')}
    tabs, tiles = [], []
    for index, (name, note, entries) in enumerate(sections):
        tab_id = old_tabs.get(name)
        if tab_id is None:
            tab_id = next((old_tabs[alias] for alias in TAB_ALIASES.get(name, ())
                           if alias in old_tabs), -index-1)
        tabs.append({'id': tab_id, 'name': name})
        old_note = old_notes.get(tab_id)
        if not old_note and index == 0:
            old_note = old_notes.get(None)
        tiles.append({'id': old_note['id'] if old_note else -1000-index,
                      'card_id': None, 'dashboard_tab_id': tab_id,
                      'row': 0, 'col': 0, 'size_x': 24, 'size_y': 3,
                      'series': [], 'parameter_mappings': [],
                      'visualization_settings': {
                          'virtual_card': {'name': None, 'display': 'text', 'visualization_settings': {}},
                          'text': navigation + '\n\n' + note}})
        for card_id, row, col, width, height in entries:
            tiles.append(_tile(by_card[card_id], tab_id, row+3, col, width, height))
    return {'tabs': tabs, 'dashcards': tiles}


def _apply(api, dashboard_id, sections, navigation):
    current = api.call('GET', f'/dashboard/{dashboard_id}')
    api.call('PUT', f'/dashboard/{dashboard_id}', layout(current, sections, navigation))
    persisted = api.call('GET', f'/dashboard/{dashboard_id}')
    return {tab['name']: tab['id'] for tab in persisted['tabs']}


def organize(api, result, activity_result):
    """Organize freshly provisioned questions without changing existing URLs."""
    dashboards, cards = result['dashboards'], result['cards']
    health = dashboards['Health overview']
    usage = activity_result['dashboard_id']
    navigation = '**Synthetic demo data.**' if os.environ.get('EXPLORER_DEMO') == '1' else ''
    tabs = {}
    tabs['reports'] = _apply(api, health, [
        ('Overview', 'Error impact in this period. Open **Problems** to investigate.', [
            (cards['count'],0,0,8,4),(cards['issues_count'],0,8,8,4),(cards['identities'],0,16,8,4),
            (cards['trend'],4,0,16,8),(cards['platforms'],4,16,8,8)]),
        ('Problems', f'Choose a problem, then an event to read its stack trace. [Releases and devices](/dashboard/{dashboards["Releases and devices"]}) · [Sessions and logs](/dashboard/{dashboards["Sessions and logs"]})', [
            (cards['issue_activity'],0,0,24,12)]),
    ], navigation)
    detail = result.get('details')
    if detail:
        ids = detail['card_ids']
        if len(ids) != 6:
            raise ProvisionError('Event detail layout requires summary, stack, breadcrumbs, logs, attachments and metadata')
        tabs['event'] = _apply(api, detail['dashboard_id'], [
            ('Stack trace', 'This event and its captured stack trace.', [
                (ids[0],0,0,24,5),(ids[1],5,0,24,10)]),
            ('Breadcrumbs', 'Captured breadcrumbs and logs with a matching trace.', [
                (ids[2],0,0,24,9),(ids[3],9,0,24,8)]),
            ('Files', 'Captured files and image previews. Administrator access required.', [
                (ids[4],0,0,24,12)]),
            ('Context', 'Device, release, tags and original context for this event.', [
                (ids[5],0,0,24,12)]),
        ], navigation)
    a = activity_result['cards']
    comparison = a.get('Activity compared with previous period')
    comparison_tiles = [(comparison,22,0,24,5)] if comparison else []
    quality_tiles = [(a['Collection health'],0,8,16,10)] if 'Collection health' in a else []
    tabs['usage'] = _apply(api, usage, [
        ('Overview', 'Installation IDs approximate players. First observed is not a store download. Choose day, week or month.', [
            (a['Active installations in period'],3,0,8,4),
            (a['First observed installations in period'],3,8,8,4),
            (a['Sessions started in period'],3,16,8,4),
            (a['Active installations over time'],7,0,12,8),
            (a['Sessions started over time by system'],7,12,12,8),
            (a['First observed installations over time'],15,0,24,7)] + comparison_tiles + ([(a['Collection status'],0,0,24,3)] if 'Collection status' in a else [])),
        ('Platforms', 'Audience and sessions by operating system. Unmatched sessions stay Unknown.', [
            (a['Active installations by system'],0,0,12,8),
            (a['Sessions started by system'],0,12,12,8),
            (a['Active installations over time by system'],8,0,24,8)]),
        ('Devices', 'Installations by device and version. The same installation may appear in several groups.', [
            (a['Active installations by system version'],0,0,12,8),
            (a['Active installations by device model'],0,12,12,8),
            (a['Active installations by app version'],8,0,24,7)]),
        ('Quality', 'Open sessions have no terminal update; they are not concurrent players. Check missing IDs and system matches.', [
            (a['Started sessions still open'],0,0,8,4),
            (a['Session outcomes by start period'],10,0,24,8),
            (a['Activity collection coverage'],18,0,24,6)] + quality_tiles),
    ], navigation)
    # Secondary dashboards keep their established layout and scopes, but share
    # the same primary destinations rather than a growing navigation list.
    for title, dashboard_id in dashboards.items():
        if dashboard_id == health:
            continue
        current = api.call('GET', f'/dashboard/{dashboard_id}')
        if current.get('public_uuid'):
            raise ProvisionError('Explorer workspace must remain authenticated')
        tiles = []
        for tile in current.get('dashcards', []):
            value = _tile(tile, tile.get('dashboard_tab_id'), tile['row'], tile['col'], tile['size_x'], tile['size_y'])
            if not tile.get('card_id'):
                instruction = 'Select an event message or ID to open its stack trace.' if title == 'Event investigation' else 'Use the dashboard filters to inspect the selected scope.'
                value['visualization_settings']['text'] = navigation + '\n\n' + instruction
            tiles.append(value)
        api.call('PUT', f'/dashboard/{dashboard_id}', {'tabs': current.get('tabs', []), 'dashcards': tiles})
    return {'reports_dashboard_id': health, 'usage_dashboard_id': usage, 'tabs': tabs}
