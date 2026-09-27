"""Generic clients must retain their classification without game-specific tags."""
import sys
import unittest
from detail_charts import specifications
from test_detail_charts import FIXTURES


def sql():
    query = specifications()[0]['query']
    for key, value in {'project_id':'1','from':"'2026-09-26'",'until':"'2026-09-27'",'layer':"'python'",'app_version':"'All'",'device_model':"'All'"}.items():
        query = query.replace('{{'+key+'}}',value)
    fixture = FIXTURES.replace("(1,'native')", "(1,'python')").replace('(1,1,1),(1,2,4)', '(1,2,4)')
    print('BEGIN READ ONLY;')
    print(f"WITH {fixture}, actual AS ({query}) SELECT 1 / (coalesce(sum(reports),0)=1)::int AS runtime_without_custom_tags FROM actual;")
    print('ROLLBACK;')


class GenericClients(unittest.TestCase):
    def test_no_sdk_or_component_allowlist(self):
        query = specifications()[0]['query']
        self.assertNotIn("'godot','flutter','rust','native'",query)
        self.assertNotIn("'gameplay','creative'",query)
        self.assertNotIn('Other exception type',query)

if __name__ == '__main__':
    if sys.argv[1:] == ['--sql']: sql()
    else: unittest.main()
