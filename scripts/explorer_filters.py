"""Readable project choices and date-range filters for native Explorer questions."""
import uuid

from provision_metabase import MARKER, ProvisionError, rows, unique_managed

PERIOD_CTE = '''WITH selected_days AS (
 SELECT day FROM crash_cache_explorer.calendar WHERE {{period}}
), selected_bounds AS (
 SELECT CASE WHEN count(*)=max(day)+1-min(day)
 THEN min(day)::timestamp AT TIME ZONE 'UTC' END AS since,
 CASE WHEN count(*)=max(day)+1-min(day)
 THEN (max(day)+1)::timestamp AT TIME ZONE 'UTC' END AS until
 FROM selected_days
), parameters AS ('''


def period_sql(sql, scalar=False):
    """Keep the tested audience SQL while replacing simple dates with field bounds."""
    if not sql.startswith('WITH parameters AS ('):
        raise ProvisionError('Explorer date wrapping requires the common parameter CTE')
    sql = sql.replace('WITH parameters AS (', PERIOD_CTE, 1)
    for name, column in (('from','since'),('until','until')):
        old = "({{" + name + "}}::date::timestamp AT TIME ZONE 'UTC')"
        if old not in sql:
            raise ProvisionError('Missing audience date parameter: ' + name)
        sql = sql.replace(old, '(SELECT ' + column + ' FROM selected_bounds)')
    if scalar:
        sql += '\nHAVING EXISTS (SELECT 1 FROM parameters WHERE since<until AND grain IS NOT NULL)'
    return sql


def configuration(api, database_id, collection_id):
    tables = api.call('GET', f'/database/{database_id}/metadata')['tables']
    by_name = {t['name']:t for t in tables if t.get('schema')=='crash_cache_explorer'}
    if 'calendar' not in by_name:
        raise ProvisionError('Install the calendar view in investigation.sql and synchronize Metabase')
    projects = by_name['projects']
    fields = {f['name']:f['id'] for f in projects['fields']}
    day = next(f['id'] for f in by_name['calendar']['fields'] if f['name']=='day')
    name = 'Project choices'
    old = unique_managed(rows(api.call('GET','/card')), name, collection_id)
    if old and old.get('public_uuid'):
        raise ProvisionError('Project choices must remain authenticated')
    value_field, label_field = ['field',fields['id'],None], ['field',fields['name'],None]
    body = {'name':name, 'description':MARKER + ' Project names for authenticated filter choices.',
            'collection_id':collection_id, 'display':'table', 'visualization_settings':{},
            'dataset_query':{'database':database_id,'type':'query','query':{
                'source-table':projects['id'],'fields':[value_field,label_field],
                'order-by':[['asc',label_field]]}}}
    card=api.call('PUT', f"/card/{old['id']}",body) if old else api.call('POST','/card',body)
    return {'day_field':day, 'project_source':{
        'values_query_type':'list','values_source_type':'card',
        'values_source_config':{'card_id':card['id'],'value_field':value_field,'label_field':label_field}}}


def tags(config, project_id, period, grain='day'):
    basic = {name:{'id':str(uuid.uuid5(uuid.NAMESPACE_URL,'explorer:'+name)),
                   'name':name,'display-name':label,'type':kind,'required':True,'default':value}
             for name,label,kind,value in [('project_id','Project','number',project_id),('grain','Group by','text',grain)]}
    basic['period']={'id':str(uuid.uuid5(uuid.NAMESPACE_URL,'explorer:period')),'name':'period',
                     'display-name':'Period (UTC)','type':'dimension','dimension':['field',config['day_field'],None],
                     'widget-type':'date/all-options','required':True,'default':period}
    return basic


def target(name):
    return ['dimension' if name=='period' else 'variable',['template-tag',name]]


def query_parameters(project_id, period, grain='day'):
    return [{'id':str(uuid.uuid5(uuid.NAMESPACE_URL,'explorer:'+name)),'type':kind,'target':target(name),'value':value}
            for name,kind,value in [('project_id','number',project_id),('period','date/all-options',period),('grain','category',grain)]]


def dashboard_parameters(config, project_id, period, grain='day'):
    return [
        {'id':'project_id','name':'Project','slug':'project','type':'number/=','default':[project_id],
         'required':True,'isMultiSelect':False,**config['project_source']},
        {'id':'period','name':'Period (UTC)','slug':'period','type':'date/all-options','default':period,'required':True},
        {'id':'grain','name':'Group by','slug':'grain','type':'category','default':[grain],
         'required':True,'isMultiSelect':False,'values_query_type':'list','values_source_type':'static-list',
         'values_source_config':{'values':[['hour','Hour'],['day','Day'],['week','Week'],['month','Month']]}}
    ]
