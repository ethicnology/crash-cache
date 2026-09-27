"""Provision authenticated event evidence; aggregate exploration stays in MBQL."""
import json
import uuid
from urllib.parse import urlsplit

from provision_explorer import preserve_dashboard_tabs
from provision_metabase import MARKER, ProvisionError, rows, unique_managed


def definitions(origin):
    parsed = urlsplit(origin)
    if origin and (parsed.scheme not in ('http', 'https') or not parsed.netloc
            or parsed.username or parsed.password or parsed.path not in ('', '/')
            or parsed.query or parsed.fragment or "'" in origin):
        raise ProvisionError('Evidence origin must be an HTTP(S) origin without credentials')
    scope = 'project_id={{project_id}} AND id={{report_id}}'
    child_scope = 'project_id={{project_id}} AND report_id={{report_id}}'
    return [
        ('Event context', f'''SELECT exception_type AS "Error",message AS "Message",event_at AS "When",
CASE WHEN jsonb_array_length(CASE WHEN jsonb_typeof(stack_frames)='array' THEN stack_frames ELSE '[]'::jsonb END)>0
THEN jsonb_array_length(stack_frames)::text || ' captured frames'
ELSE 'No stack trace received for this event' END AS "Stack trace"
FROM crash_cache_explorer.reports WHERE {scope}''', 'object', (3, 0, 24, 5)),
        ('Stack trace', f'''SELECT position AS frame,function,
concat_ws(':',filename,line_number) AS location,source_line AS source
FROM crash_cache_explorer.frames WHERE {child_scope} ORDER BY position''', 'table', (8, 0, 24, 7)),
        ('Breadcrumb timeline', f'''SELECT chronology,event_at,category,type,level,message,data
FROM crash_cache_explorer.breadcrumbs WHERE {child_scope} ORDER BY chronology''', 'table', (15, 0, 24, 7)),
        ('Correlated structured logs', '''SELECT l.event_at,l.level,l.body,l.trace_id,l.span_id,l.attributes
FROM crash_cache_explorer.report_logs r JOIN crash_cache_explorer.logs l ON l.id=r.log_id
AND l.project_id=r.project_id WHERE r.project_id={{project_id}} AND r.report_id={{report_id}}
ORDER BY l.event_at,l.id''', 'table', (22, 0, 24, 6)),
        ('Event attachments', f'''SELECT id,filename,content_type,size_bytes,
'{origin.rstrip('/')}/api/evidence/attachments/' || id AS evidence_url,
CASE WHEN content_type IN ('image/png','image/jpeg') THEN
'{origin.rstrip('/')}/api/evidence/attachments/' || id END AS image_preview
FROM crash_cache_explorer.attachments WHERE {child_scope} ORDER BY id''', 'table', (28, 0, 24, 5)),
        ('Event metadata', f'''SELECT id,event_id,platform,environment,app_version,device_model,
os_name,os_version,identity,layer,component,trace_id,tags,custom_contexts,stack_frames
FROM crash_cache_explorer.reports WHERE {scope}''', 'object', (33, 0, 24, 10)),
    ]


def provision(api, database_id, collection_id, project_id, report_id, origin, home_id):
    cards = rows(api.call('GET', '/card'))
    selected = []
    tags = {name: {'id': str(uuid.uuid5(uuid.NAMESPACE_URL, 'explorer:' + name)),
                   'name': name, 'display-name': label, 'type': 'number',
                   'required': True, 'default': value}
            for name, label, value in [('project_id', 'Project', project_id), ('report_id', 'Event record', report_id)]}
    parameters = [{'id': name, 'type': 'number/=', 'target': ['variable', ['template-tag', name]], 'value': value}
                  for name, value in [('project_id', project_id), ('report_id', report_id)]]
    for name, sql, display, layout in definitions(origin):
        old = unique_managed(cards, name, collection_id)
        if old is None and name == 'Stack trace':
            old = unique_managed(cards, 'Stack frames', collection_id)
        if old and old.get('public_uuid'):
            raise ProvisionError('Event evidence questions must not be publicly shared')
        settings = {}
        if name == 'Event attachments':
            settings = {'column_settings': {'["name","evidence_url"]': {
                'column_title': 'Open attachment', 'view_as': 'link', 'link_text': 'View / download',
                'link_url': '{{evidence_url}}'}, '["name","image_preview"]': {
                'column_title': 'Preview', 'view_as': 'image'}}}
        body = {'name': name, 'description': MARKER + ' Authenticated raw evidence for one project and event.',
                'collection_id': collection_id, 'display': display, 'visualization_settings': settings,
                'dataset_query': {'database': database_id, 'type': 'native',
                                  'native': {'query': sql, 'template-tags': tags}}}
        card = api.call('PUT', f"/card/{old['id']}", body) if old else api.call('POST', '/card', body)
        result = api.call('POST', f"/card/{card['id']}/query", {'parameters': parameters})
        if result.get('status') != 'completed':
            raise ProvisionError('Event detail query failed: ' + name)
        if display == 'table':
            columns = settings.setdefault('column_settings', {})
            for column in result.get('data', {}).get('cols', []):
                key = json.dumps(['name', column['name']], separators=(',', ':'))
                columns.setdefault(key, {})['column_title'] = {
                    'evidence_url': 'Open attachment', 'image_preview': 'Preview',
                    'line_number': 'Line', 'source_line': 'Source', 'size_bytes': 'Bytes',
                }.get(column['name'], column['name'].replace('_', ' ').capitalize())
            api.call('PUT', f"/card/{card['id']}", {'visualization_settings': settings})
        selected.append((card, layout))
    name = 'Event details'
    dashboard = unique_managed(rows(api.call('GET', '/dashboard')), name, collection_id)
    if not dashboard:
        dashboard = api.call('POST', '/dashboard', {'name': name, 'description': MARKER, 'collection_id': collection_id})
    current = api.call('GET', f"/dashboard/{dashboard['id']}")
    if current.get('public_uuid'):
        raise ProvisionError('Event details must not be publicly shared; remove sharing before provisioning')
    old_tiles = {tile['card_id']: tile['id'] for tile in current.get('dashcards', [])}
    tiles = [{'id': old_tiles.get(None, -100), 'card_id': None, 'row': 0, 'col': 0, 'size_x': 24, 'size_y': 3,
              'series': [], 'parameter_mappings': [], 'visualization_settings': {
                  'virtual_card': {'name': None, 'display': 'text', 'visualization_settings': {}},
                  'text': f'### Event evidence\n[Back to health](/dashboard/{home_id}) · Project and event record select the exact occurrence. Empty sections mean no matching evidence was retained. Attachment access requires an active administrator.'}}]
    for index, (card, (row, col, width, height)) in enumerate(selected):
        tiles.append({'id': old_tiles.get(card['id'], -index-1), 'card_id': card['id'],
                      'row': row, 'col': col, 'size_x': width, 'size_y': height, 'series': [],
                      'visualization_settings': {}, 'parameter_mappings': [
                          {'parameter_id': name, 'card_id': card['id'], 'target': ['variable', ['template-tag', name]]}
                          for name in tags]})
    api.call('PUT', f"/dashboard/{dashboard['id']}", {**preserve_dashboard_tabs(current, tiles), 'parameters': [
        {'id': name, 'name': label, 'slug': slug, 'type': 'number/=', 'default': [value], 'required': True, 'isMultiSelect': False}
        for name, label, slug, value in [('project_id', 'Project', 'project', project_id), ('report_id', 'Event record', 'event', report_id)]]})
    return {'dashboard_id': dashboard['id'], 'card_ids': [card['id'] for card, _ in selected]}


def link_events(api, dashboard_id, event_card_id, detail_id):
    """Link event IDs to the evidence dashboard; retain the selected project."""
    current = api.call('GET', f'/dashboard/{dashboard_id}')
    tiles = []
    for tile in current['dashcards']:
        visual = dict(tile.get('visualization_settings') or {})
        if tile.get('card_id') == event_card_id:
            visual['click_behavior'] = {'type': 'link', 'linkType': 'url',
                'linkTemplate': f'/dashboard/{detail_id}?project={{{{project}}}}&event={{{{id}}}}&period={{{{period}}}}'}
            settings = dict(visual.get('column_settings') or {})
            settings['["name","id"]'] = {'column_title': 'Open event / stack trace', 'click_behavior': {'type': 'link', 'linkType': 'url',
                'linkTemplate': f'/dashboard/{detail_id}?project={{{{project}}}}&event={{{{id}}}}&period={{{{period}}}}'}}
            visual['column_settings'] = settings
        tiles.append({**{key: tile[key] for key in ('id','card_id','row','col','size_x','size_y','parameter_mappings')},
                      'series': [], 'visualization_settings': visual})
    api.call('PUT', f'/dashboard/{dashboard_id}', preserve_dashboard_tabs(current, tiles))


def link_issues(api, dashboard_id, issue_card_id, investigation_id):
    current = api.call('GET', f'/dashboard/{dashboard_id}')
    tiles = []
    for tile in current['dashcards']:
        visual = dict(tile.get('visualization_settings') or {})
        if tile.get('card_id') == issue_card_id:
            visual['click_behavior'] = {'type': 'link', 'linkType': 'url', 'linkTemplate':
                f'/dashboard/{investigation_id}?project={{{{project}}}}&period={{{{period}}}}'
                '&environment={{environment}}&version={{version}}&issue={{issue_key}}'}
        tiles.append({**{key: tile[key] for key in ('id','card_id','row','col','size_x','size_y','parameter_mappings')},
                      'series': [], 'visualization_settings': visual})
    api.call('PUT', f'/dashboard/{dashboard_id}', preserve_dashboard_tabs(current, tiles))
