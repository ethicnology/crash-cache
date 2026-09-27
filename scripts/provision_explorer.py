#!/usr/bin/env python3
"""Provision authenticated query-builder investigation on Metabase 0.63.18.2.

Views must already be installed and database metadata synchronized. The supplied
API may already be authenticated; this module never creates users or publishes
links. Standalone questions cover authorized database rows; dashboard parameters
provide the selected project and period. Permissions remain operator-owned.
"""
import argparse
import datetime as dt
import json
import os

from provision_metabase import Api, ProvisionError, required, rows

MARKER = 'Managed by crash-cache explorer provisioning.'
COLLECTION = 'Crash-cache Explorer'
SCHEMA = 'crash_cache_explorer'
TABLES = ('projects','issues','reports','frames','breadcrumbs','attachments','logs','report_logs','sessions')
DESCRIPTIONS = {
    'issues': 'Lifetime issue summaries, isolated by project. Use the issue activity question for filtered counts.',
    'reports': 'Individual stored events with stack frames, tags and context. Select an event ID to inspect its record.',
    'frames': 'One stack frame per row. Position follows the original captured stack order.',
    'breadcrumbs': 'Captured breadcrumbs. Sort chronology ascending; unknown timestamps follow dated entries.',
    'attachments': 'Attachment metadata only; a listed file is not an embedded download.',
    'logs': 'Structured logs, including logs without a matching error report.',
    'report_logs': 'Explicit trace and project matches. Several reports may reference the same log.',
    'sessions': 'SDK sessions and their latest reported status. Identity is not necessarily a person or installation.',
    'projects': 'Projects without ingestion keys.',
}
FOREIGN_KEYS = {
    'issues': {'project_id':('projects','id'), 'latest_report_id':('reports','id')},
    'reports': {'project_id':('projects','id'), 'issue_key':('issues','id'), 'session_id':('sessions','id')},
    'frames': {'project_id':('projects','id'), 'issue_key':('issues','id'), 'report_id':('reports','id')},
    'breadcrumbs': {'project_id':('projects','id'), 'issue_key':('issues','id'), 'report_id':('reports','id')},
    'attachments': {'project_id':('projects','id'), 'issue_key':('issues','id'), 'report_id':('reports','id')},
    'logs': {'project_id':('projects','id')},
    'report_logs': {'project_id':('projects','id'), 'issue_key':('issues','id'), 'report_id':('reports','id'), 'log_id':('logs','id')},
    'sessions': {'project_id':('projects','id')},
}
LABELS = {'id':'Record ID','issue_key':'Issue','project_id':'Project','event_at':'Event time (UTC)',
          'event_count':'Recorded events','identity':'Recorded identity','affected_identities':'Affected identities',
          'reports_without_identity':'Events without identity','stack_frames':'Captured stack frames',
          'custom_contexts':'Custom context','source_issue_id':'Storage issue ID','trace_id':'Trace ID',
          'report_id':'Event record','log_id':'Log record','latest_report_id':'Latest event',
          'app_version':'App version','first_seen':'First seen (UTC)','last_seen':'Last seen (UTC)'}


def managed(items, name, collection_id=None):
    matches = [x for x in items if x.get('name') == name and
               (collection_id is None or x.get('collection_id') == collection_id)]
    if len(matches) > 1:
        raise ProvisionError(f'Ambiguous Explorer object: {name}')
    if matches and matches[0].get('public_uuid'):
        raise ProvisionError(f'Refusing shared Explorer object containing diagnostic content: {name}')
    if matches and not (matches[0].get('description') or '').startswith(MARKER):
        raise ProvisionError(f'Refusing to overwrite unmanaged Explorer object: {name}')
    return matches[0] if matches else None


def field(metadata, table, name, **options):
    return ['field', metadata[table]['fields'][name]['id'], options or None]


def metadata_for(api, database_id):
    metadata = api.call('GET', f'/database/{database_id}/metadata')
    result = {}
    for table in metadata['tables']:
        if table.get('schema') == SCHEMA and table['name'] in TABLES:
            result[table['name']] = {**table, 'fields': {f['name']:f for f in table['fields']}}
    if set(result) != set(TABLES):
        raise ProvisionError('Install investigation.sql and synchronize database metadata before provisioning Explorer')
    return result


def configure_metadata(api, metadata):
    for table_name, table in metadata.items():
        api.call('PUT', f"/table/{table['id']}", {'display_name':table_name.replace('_',' ').title(),
                 'description':DESCRIPTIONS[table_name]})
        for name, item in table['fields'].items():
            body = {'display_name':LABELS.get(name,name.replace('_',' ').capitalize())}
            if name == 'id':
                body['semantic_type'] = 'type/PK'
            elif name in FOREIGN_KEYS.get(table_name, {}):
                target_table, target_field = FOREIGN_KEYS[table_name][name]
                body.update(semantic_type='type/FK', fk_target_field_id=metadata[target_table]['fields'][target_field]['id'])
            elif name in ('environment','platform','app_version','device_model','exception_type','layer','component','status','level'):
                body['semantic_type'] = 'type/Category'
            if name in ('stack_frames','custom_contexts','tags','variables','frame','data','attributes'):
                body['description'] = 'Captured diagnostic content; available to authorized users only.'
            api.call('PUT', f"/field/{item['id']}", body)


def definitions(metadata, database_id):
    def f(name, table='reports', **opts):
        return field(metadata,table,name,**opts)
    def card(key,name,table,display,query,description,settings=None):
        return {'key':key,'name':name,'table':table,'display':display,
                'dataset_query':{'database':database_id,'type':'query','query':{'source-table':metadata[table]['id'],**query}},
                'description':description,'visualization_settings':settings or {}}
    errors = ['not-null',f('issue_key')]
    result = [
        card('count','Error events','reports','scalar',{'filter':errors,'aggregation':[['count']]},'Stored error occurrences in the selected scope.'),
        card('issues_count','Distinct problems','reports','scalar',{'filter':errors,'aggregation':[['distinct',f('issue_key')]]},'Different grouped problems with at least one event in the selected scope.'),
        card('identities','Affected identities','reports','scalar',{'filter':['and',errors,['not-null',f('identity')],['!=',f('identity'),'']], 'aggregation':[['distinct',f('identity')]]},'Distinct nonempty identities on error events; not a count of people.'),
        card('trend','Errors over time','reports','line',{'filter':errors,'aggregation':[['count']], 'breakout':[f('event_at',**{'temporal-unit':'day'})]},'Daily error events in UTC. Click a point to explore its records.'),
        card('platforms','Errors by platform','reports','bar',{'filter':errors,'aggregation':[['count']], 'breakout':[f('platform')], 'order-by':[['desc',['aggregation',0]]]},'Compare affected runtimes, then click to inspect events.'),
        card('issue_activity','Problems to investigate','reports','table',{'filter':errors,
             'aggregation':[['count'],['distinct',f('identity')],['min',f('event_at')],['max',f('event_at')]],
             'breakout':[field(metadata,'issues','title',**{'source-field':metadata['reports']['fields']['issue_key']['id']}),f('issue_key')],
             'order-by':[['desc',['aggregation',1]],['desc',['aggregation',0]]]},'Counts and first/last times are recalculated after dashboard filters. Select an issue to inspect its lifetime record.'),
        card('events','Event records','reports','table',{'filter':errors,'fields':[f(n) for n in ('title','event_at','app_version','platform','device_model','environment','id','identity','issue_key')], 'order-by':[['desc',f('event_at')],['desc',f('id')]],'limit':200},'Most recent 200 errors in scope. Select a message or event ID to open its stack trace. Open the question to change its limit.'),
        card('sessions','Session records','sessions','table',{'fields':[f(n,'sessions') for n in ('id','started_at','status','release','environment','identity','errors','duration')], 'order-by':[['desc',f('started_at','sessions')]],'limit':200},'Most recent SDK sessions; open and abnormal states remain explicit.'),
        card('session_status','Session status','sessions','bar',{'aggregation':[['count']],'breakout':[f('status','sessions')]},'Counts of recorded sessions by their latest reported status. This is not crash-free user coverage.'),
        card('logs','Log records','logs','table',{'fields':[f(n,'logs') for n in ('id','event_at','level','body','trace_id')], 'order-by':[['desc',f('event_at','logs')]],'limit':200},'Most recent 200 structured logs. Trace correlation requires an actual matching trace.'),
        card('log_levels','Log severity','logs','bar',{'aggregation':[['count']],'breakout':[f('level','logs')]},'Structured log counts in the selected project and period.'),
        card('versions','Errors by app version','reports','bar',{'filter':errors,'aggregation':[['count']], 'breakout':[f('app_version')], 'order-by':[['desc',['aggregation',0]]]},'Error occurrence counts by application version. Counts are not normalized by version adoption.'),
        card('devices','Most affected device models','reports','bar',{'filter':errors,'aggregation':[['count']], 'breakout':[f('device_model')], 'order-by':[['desc',['aggregation',0]]],'limit':10},'Top ten device models by recorded error occurrences, not unique devices or failure rates.'),
        card('release_outcomes','Session outcomes by release','sessions','table',{'aggregation':[['count']], 'breakout':[f('release','sessions'),f('status','sessions')], 'order-by':[['desc',['aggregation',0]]]},'Recorded session counts by full SDK release and latest reported status. Open sessions remain visible.'),
    ]
    for definition in result:
        settings = definition['visualization_settings']
        if definition['display'] in ('line', 'bar'):
            settings.update({'graph.colors':['#D95D67'] if definition['table']=='reports' else ['#258A88'],
                             'graph.y_axis.title_text':'Recorded events' if definition['table']=='reports' else 'Records'})
        if definition['key']=='issue_activity':
            settings['column_settings'] = {
                json.dumps(['name',name],separators=(',',':')):{'column_title':title}
                for name,title in [('issue_key','Technical problem ID'),('title','Problem'),('count','Occurrences'),
                                   ('count_2','Installations affected'),('min','First occurrence'),('max','Latest occurrence')]}
            settings['table.columns'] = [{'name':name,'enabled':True} for name in ('title','count_2','count','max','min','issue_key')]
    return result


def dashboard_parameters(project_id, start, end, dashboard_name=None):
    parameters = [{'id':'project','name':'Project','slug':'project','type':'number/=','default':[project_id]},
            {'id':'period','name':'Period (UTC)','slug':'period','type':'date/all-options','default':f'{start}~{end}'},
            {'id':'environment','name':'Environment','slug':'environment','type':'string/='},
            {'id':'version','name':'Version','slug':'version','type':'string/='}]
    if dashboard_name == 'Sessions and logs':
        parameters[-1] = {'id':'release','name':'Release','slug':'release','type':'string/='}
    if dashboard_name == 'Releases and devices':
        parameters.append({'id':'release','name':'Release','slug':'release','type':'string/='})
    if dashboard_name == 'Event investigation':
        parameters.append({'id':'issue','name':'Issue','slug':'issue','type':'number/='})
    return parameters


def mappings(metadata, definition, card_id, dashboard_name=None):
    table = definition['table']
    columns = {'project':'project_id','period':'started_at' if table=='sessions' else 'event_at'}
    if table in ('reports','sessions'):
        columns['environment']='environment'
        columns['release' if table=='sessions' else 'version']='release' if table=='sessions' else 'app_version'
    if dashboard_name=='Event investigation' and table=='reports':
        columns['issue']='issue_key'
    return [{'parameter_id':parameter,'card_id':card_id,'target':['dimension',field(metadata,table,column)]}
            for parameter,column in columns.items()]


def preserve_dashboard_tabs(current, tiles):
    """Keep tab identities during intermediate layout and link updates.

    Metabase resets tabs when a dashcard PUT omits them. Every tile must also
    reference a valid tab when tabs exist. New tiles temporarily use the first
    tab until the workspace organizer applies their final section.
    """
    tabs = [{'id': tab['id'], 'name': tab['name']} for tab in current.get('tabs', [])]
    by_id = {tile['id']: tile.get('dashboard_tab_id') for tile in current.get('dashcards', [])}
    by_card = {tile['card_id']: tile.get('dashboard_tab_id')
               for tile in current.get('dashcards', []) if tile.get('card_id')}
    default = tabs[0]['id'] if tabs else None
    allowed = {tab['id'] for tab in tabs}
    for tile in tiles:
        tab_id = by_id.get(tile['id'], by_card.get(tile.get('card_id'), default))
        tile['dashboard_tab_id'] = tab_id if tab_id in allowed else default
    return {'tabs': tabs, 'dashcards': tiles}


def provision(api, project_id, start=None, end=None, database_id=2):
    today = dt.datetime.now(dt.timezone.utc).date()
    start = start or (today-dt.timedelta(days=20)).isoformat()
    end = end or today.isoformat()
    if project_id<=0 or dt.date.fromisoformat(start)>dt.date.fromisoformat(end):
        raise ProvisionError('Provide a positive project and ordered ISO dates')
    if not api.session:
        api.session=api.call('POST','/session',{'username':required('METABASE_ADMIN_EMAIL'),'password':required('METABASE_ADMIN_PASSWORD')})['id']
    metadata=metadata_for(api,database_id)
    configure_metadata(api,metadata)
    collection=managed(rows(api.call('GET','/collection')),COLLECTION)
    if not collection:
        collection=api.call('POST','/collection',{'name':COLLECTION,'description':MARKER})
    collection_id=collection['id']
    existing=rows(api.call('GET','/card'))
    cards={}
    specs=definitions(metadata,database_id)
    for definition in specs:
        current=managed(existing,definition['name'],collection_id)
        body={k:v for k,v in definition.items() if k not in ('key','table')}
        body.update(collection_id=collection_id,description=MARKER+' '+definition['description'])
        card=api.call('PUT',f"/card/{current['id']}",body) if current else api.call('POST','/card',body)
        cards[definition['key']]=card['id']
    dashboards={}
    layouts={
        'Health overview':[('count',0,0,8,3),('issues_count',0,8,8,3),('identities',0,16,8,3),('trend',3,0,16,7),('platforms',3,16,8,7),('issue_activity',10,0,24,9)],
        'Event investigation':[('issue_activity',0,0,24,8),('events',8,0,24,10)],
        'Sessions and logs':[('session_status',0,0,12,6),('log_levels',0,12,12,6),('sessions',6,0,24,8),('logs',14,0,24,8)],
        'Releases and devices':[('versions',0,0,12,7),('devices',0,12,12,7),('release_outcomes',7,0,24,9)],
    }
    current_dashboards=rows(api.call('GET','/dashboard'))
    for name in layouts:
        dashboard=managed(current_dashboards,name,collection_id)
        if not dashboard:
            dashboard=api.call('POST','/dashboard',{'name':name,'description':MARKER,'collection_id':collection_id})
        dashboards[name]=dashboard['id']
    for name,layout in layouts.items():
        dashboard={'id':dashboards[name]}
        current=api.call('GET',f"/dashboard/{dashboard['id']}")
        if current.get('public_uuid'):
            raise ProvisionError(f'Refusing shared Explorer dashboard: {name}')
        old={d['card_id']:d['id'] for d in current.get('dashcards',[]) if d.get('card_id')}
        links=' | '.join(f'[{title}](/dashboard/{identifier})' for title,identifier in dashboards.items())
        banner='**Synthetic demonstration data; not production measurements.**' if os.environ.get('EXPLORER_DEMO')=='1' else '**Explore recorded application diagnostics.**'
        note=f'{banner}\n\n{links}\n\nSection links use their own saved/default filters. Chart drill-through retains the selected scope.'
        if name=='Event investigation':
            note+=' **Open a stack trace:** select an event message or its ID in Event records below. Activity observations are excluded.'
        if name=='Releases and devices':
            note+=' Version filters error charts; Release filters session outcomes. Project, period and environment apply to all three.'
        note_id=next((d['id'] for d in current.get('dashcards',[]) if d.get('card_id') is None),-1000)
        dashcards=[{'id':note_id,'card_id':None,'row':0,'col':0,'size_x':24,'size_y':3,'series':[], 'parameter_mappings':[], 'visualization_settings':{'virtual_card':{'name':None,'display':'text','visualization_settings':{}},'text':note}}]
        for index,(key,row,col,width,height) in enumerate(layout):
            definition=next(d for d in specs if d['key']==key)
            dashcards.append({'id':old.get(cards[key],-index-1),'card_id':cards[key], 'row':row+3,'col':col,'size_x':width,'size_y':height,'series':[], 'visualization_settings':{},'parameter_mappings':mappings(metadata,definition,cards[key],name)})
        description=MARKER+' Select a chart to drill into records. Period includes both dates. Identities are SDK identifiers, not verified people.'
        if name=='Sessions and logs':
            description+=' Environment and release apply to sessions only; logs have no normalized environment/release columns.'
        api.call('PUT',f"/dashboard/{dashboard['id']}",{'description':description,'parameters':dashboard_parameters(project_id,start,end,name),**preserve_dashboard_tabs(current,dashcards)})
        dashboards[name]=dashboard['id']
    return {'collection_id':collection_id,'dashboards':dashboards,'cards':cards,
            'tables':{name:value['id'] for name,value in metadata.items()},'project_id':project_id,
            'start':start,'end':end,'database_id':database_id}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project',type=int,default=os.environ.get('DEMO_PROJECT_ID'))
    parser.add_argument('--database',type=int,default=int(os.environ.get('METABASE_DATABASE_ID','2')))
    parser.add_argument('--start')
    parser.add_argument('--end')
    parser.add_argument('--evidence-origin', help='Common HTTP(S) origin, or same-origin for relative evidence URLs; enables the event detail dashboard')
    parser.add_argument('--event',type=int,default=0,help='Initial event record for the detail dashboard; zero starts empty')
    args=parser.parse_args()
    if args.project is None:
        parser.error('--project or DEMO_PROJECT_ID is required')
    api=Api(required('METABASE_URL'))
    try:
        result=provision(api,args.project,args.start,args.end,args.database)
        from explorer_details import link_issues, link_events, provision as provision_details
        investigation=result['dashboards']['Event investigation']
        for dashboard in (result['dashboards']['Health overview'],investigation):
            link_issues(api,dashboard,result['cards']['issue_activity'],investigation)
        if args.evidence_origin is not None:
            origin='' if args.evidence_origin=='same-origin' else args.evidence_origin
            detail=provision_details(api,args.database,result['collection_id'],args.project,args.event,origin,
                                     result['dashboards']['Health overview'])
            link_events(api,investigation,result['cards']['events'],detail['dashboard_id'])
            result['details']=detail
        from provision_activity import provision as provision_activity
        from explorer_workspace import organize
        activity=provision_activity(api,args.database,result['collection_id'],args.project,result['start'],result['end'])
        result['activity']=activity
        from provision_explorer_home import provision as provision_home, named_project_filters
        result['home']=provision_home(api,result)
        named_project_filters(api,result)
        result['workspace']=organize(api,result,activity)
        from explorer_navigation import provision as provision_navigation
        result['navigation']=provision_navigation(api,result)
        print(json.dumps(result))
    finally:
        if api.session:
            api.call('DELETE','/session',{'metabase-session-id':api.session})


if __name__=='__main__':
    main()
