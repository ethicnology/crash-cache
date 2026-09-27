"""Scope-preserving primary navigation using native Metabase table-cell links."""
import json

from provision_explorer import preserve_dashboard_tabs
from provision_metabase import MARKER, ProvisionError, rows, unique_managed


def provision(api, result):
    name='Explore this selection'
    existing=unique_managed(rows(api.call('GET','/card')),name,result['collection_id'])
    if existing and existing.get('public_uuid'):
        raise ProvisionError('Navigation must remain authenticated')
    body={'name':name,'description':MARKER+' Navigate with the current project and period.',
          'collection_id':result['collection_id'],'display':'table','visualization_settings':{},
          'dataset_query':{'database':result['database_id'],'type':'native',
                           'native':{'query':"SELECT 'Overview' AS overview, 'Usage' AS usage, 'Errors' AS errors",'template-tags':{}}}}
    card=api.call('PUT',f"/card/{existing['id']}",body) if existing else api.call('POST','/card',body)
    destinations={'overview':result['home']['dashboard_id'],'usage':result['activity']['dashboard_id'],
                  'errors':result['dashboards']['Health overview']}
    settings={'column_settings':{json.dumps(['name',key],separators=(',',':')):{
        'column_title':key.capitalize(),'click_behavior':{'type':'link','linkType':'url',
        'linkTemplate':f'/dashboard/{identifier}?project={{{{project}}}}&period={{{{period}}}}'}}
        for key,identifier in destinations.items()}}
    identifiers=[*result['dashboards'].values(),result['home']['dashboard_id'],result['activity']['dashboard_id']]
    if result.get('details'):identifiers.append(result['details']['dashboard_id'])
    for identifier in identifiers:
        current=api.call('GET',f'/dashboard/{identifier}')
        if current.get('public_uuid'):raise ProvisionError('Navigation dashboard must remain authenticated')
        tabs=[t['id'] for t in current.get('tabs',[])] or [None]
        old_navigation={t.get('dashboard_tab_id'):t for t in current['dashcards'] if t.get('card_id')==card['id']}
        tiles=[]
        for tile in current['dashcards']:
            if tile.get('card_id')==card['id']:continue
            tile={**tile}
            # Remove the previous navigation gap before choosing its new position.
            previous=old_navigation.get(tile.get('dashboard_tab_id'))
            if previous and tile['row']>=previous['row']+previous['size_y']:
                tile['row']-=previous['size_y']
            tiles.append(tile)
        hero_ids={result['cards'][key] for key in ('count','identities','issues_count')}
        hero_ids.update(result['activity']['cards'][key] for key in ('Active installations in period','First observed installations in period','Sessions started in period','Collection status'))
        for index,tab_id in enumerate(tabs):
            content=[t for t in tiles if t.get('dashboard_tab_id')==tab_id]
            hero=[t for t in content if t.get('card_id') in hero_ids]
            row=max((t['row']+t['size_y'] for t in hero or content),default=0)
            for tile in content:
                if tile['row']>=row:tile['row']+=3
            tiles.append({'id':old_navigation.get(tab_id,{}).get('id',-900-index),'card_id':card['id'],
                          'row':row,'col':0,'size_x':24,'size_y':3,'dashboard_tab_id':tab_id,
                          'series':[],'parameter_mappings':[],'visualization_settings':settings})
        # Preserve chosen tab membership for the added per-tab navigation cards.
        navigation_tabs={t['id']:t.get('dashboard_tab_id') for t in tiles if t.get('card_id')==card['id']}
        preserved=preserve_dashboard_tabs(current,tiles)
        for tile in preserved['dashcards']:
            if tile['card_id']==card['id']:
                tile['dashboard_tab_id']=navigation_tabs[tile['id']]
        # A detail page retains its originating date filter for the navigation links.
        parameters=current['parameters']
        if not any(p['slug']=='period' for p in parameters):
            parameters.append({'id':'period','name':'Period (UTC)','slug':'period','type':'date/all-options',
                               'default':f"{result['start']}~{result['end']}", 'required':True})
        api.call('PUT',f'/dashboard/{identifier}',{'parameters':parameters,**preserved})
    return {'card_id':card['id']}
