"""Provide one compact entry point into usage and error investigation."""
import os

from explorer_filters import dashboard_parameters, target
from provision_explorer import definitions, mappings, metadata_for, preserve_dashboard_tabs
from provision_metabase import MARKER, ProvisionError, rows, unique_managed


def provision(api, result):
    audience=result['activity']
    a,c=audience['cards'],result['cards']
    config=audience['filter_config']
    name='Overview'
    old=unique_managed(rows(api.call('GET','/dashboard')),name,result['collection_id'])
    if old and old.get('public_uuid'):
        raise ProvisionError('Explorer overview must remain authenticated')
    dashboard=old or api.call('POST','/dashboard',{'name':name,'description':MARKER,
                                                 'collection_id':result['collection_id']})
    current=api.call('GET',f"/dashboard/{dashboard['id']}")
    old_tiles={t['card_id']:t['id'] for t in current.get('dashcards',[]) if t.get('card_id')}
    usage=audience['dashboard_id']; errors=result['dashboards']['Health overview']
    prefix='**Synthetic demo.** ' if os.environ.get('EXPLORER_DEMO')=='1' else ''
    note=prefix+'Counts reflect received telemetry, not store downloads.'
    note_id=next((t['id'] for t in current.get('dashcards',[]) if not t.get('card_id')),-1000)
    tiles=[{'id':note_id,'card_id':None,'row':0,'col':0,'size_x':24,'size_y':2,
            'series':[],'parameter_mappings':[],'visualization_settings':{
                'virtual_card':{'name':None,'display':'text','visualization_settings':{}},'text':note}}]
    native=[(a['Collection status'],5,0,24,2),
            (a['Active installations in period'],2,0,6,3),
            (a['Sessions started in period'],2,12,6,3),
            (a['Activity compared with previous period'],7,0,24,5),
            (a['Collection health'],19,0,24,8)]
    for card_id,row,col,width,height in native:
        tiles.append({'id':old_tiles.get(card_id,-len(tiles)-1),'card_id':card_id,'row':row,'col':col,
                      'size_x':width,'size_y':height,'series':[],'visualization_settings':(
                          {'card.title':'Active installations' if card_id==a['Active installations in period'] else 'Sessions started',
                           'click_behavior':{'type':'link','linkType':'url','linkTemplate':f'/dashboard/{usage}?project={{{{project}}}}&period={{{{period}}}}'}}
                          if card_id in (a['Active installations in period'],a['Sessions started in period']) else {}),
                      'parameter_mappings':[{'parameter_id':key,'card_id':card_id,'target':target(key)}
                                            for key in ('project_id','period','grain')]})
    metadata=metadata_for(api,result['database_id'])
    specs={d['key']:d for d in definitions(metadata,result['database_id'])}
    for key,row,col,width,height in [('count',2,6,6,3),('identities',2,18,6,3),
                                     ('trend',12,0,14,7),('versions',12,14,10,7)]:
        card_id=c[key]
        scope=[m for m in mappings(metadata,specs[key],card_id) if m['parameter_id'] in ('project','period')]
        for m in scope:
            if m['parameter_id']=='project':m['parameter_id']='project_id'
        tiles.append({'id':old_tiles.get(card_id,-len(tiles)-1),'card_id':card_id,'row':row,'col':col,
                      'size_x':width,'size_y':height,'series':[],'visualization_settings':(
                          {'card.title':'Errors' if key=='count' else 'Installations affected','click_behavior':{'type':'link','linkType':'url','linkTemplate':f'/dashboard/{errors}?project={{{{project}}}}&period={{{{period}}}}'}} if key in ('count','identities') else {}),'parameter_mappings':scope})
    filters=dashboard_parameters(config,result['project_id'],f"{result['start']}~{result['end']}")
    # The overview keeps daily trend charts; detailed time buckets live in Usage.
    filters=[p for p in filters if p['id']!='grain']
    for tile in tiles:
        tile['parameter_mappings']=[m for m in tile['parameter_mappings'] if m['parameter_id']!='grain']
    api.call('PUT',f"/dashboard/{dashboard['id']}",{'description':MARKER+' Start with activity and error impact, compare the preceding equally long period, then explore details.',
             'parameters':filters,**preserve_dashboard_tabs(current,tiles)})
    return {'dashboard_id':dashboard['id']}


def named_project_filters(api, result):
    """Use the same dynamic name source across every project selector."""
    source=result['activity']['filter_config']['project_source']
    ids=list(result['dashboards'].values())
    if result.get('details'):ids.append(result['details']['dashboard_id'])
    for identifier in ids:
        current=api.call('GET',f'/dashboard/{identifier}')
        for parameter in current['parameters']:
            if parameter['id'] in ('project','project_id'):
                parameter.update(source,isMultiSelect=False,required=True)
        api.call('PUT',f'/dashboard/{identifier}',{'parameters':current['parameters'],
                 **preserve_dashboard_tabs(current,current['dashcards'])})
