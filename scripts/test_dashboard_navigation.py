"""Navigation contracts independent of a running Metabase instance."""
import unittest
from dashboard_navigation import configure, drill_url
from provision_metabase import ProvisionError, query_parameters, template_tags
from detail_charts import FILTERS

class Navigation(unittest.TestCase):
    def test_drill_preserves_project_dates_and_clicked_dimension(self):
        url = drill_url('/public/dashboard/fixture', 'device_model')
        self.assertEqual(url, '/public/dashboard/fixture?project={{project}}&from={{from}}&until={{until}}&device_model={{device_model}}')

    def test_detail_parameters_share_identity_and_default(self):
        tags = template_tags(1, '2026-09-26', '2026-09-27', detail_filters=FILTERS)
        parameters = query_parameters(1, '2026-09-26', '2026-09-27', detail_filters=FILTERS)
        for parameter in parameters[-3:]:
            name = parameter['target'][1][1]
            self.assertEqual(parameter['id'], tags[name]['id'])
            self.assertEqual(parameter['value'], 'All')
            self.assertEqual(tags[name]['default'], 'All')

    def test_invalid_public_origin_is_rejected_before_network_use(self):
        for base in ['javascript:alert(1)', 'https://', 'https://host/path', 'https://user:secret@host', 'https://host?token=value']:
            with self.assertRaises(ProvisionError):
                configure(None, base)

if __name__ == '__main__':
    unittest.main()
