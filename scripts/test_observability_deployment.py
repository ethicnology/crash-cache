#!/usr/bin/env python3
"""Check the opt-in deployment against the backend startup contract."""
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


class DeploymentContractTest(unittest.TestCase):
    def test_backend_receives_every_required_setting(self):
        settings = (ROOT / 'src/config/settings.rs').read_text()
        required = set(re.findall(
            r'Self::require_env(?:_parse)?(?:_or_fallback)?\(\s*"([A-Z_]+)"', settings))
        compose = (ROOT / 'deploy/observability/compose.yml').read_text()
        backend = compose.split('\n  crash-cache:\n', 1)[1].split('\n  metabase:\n', 1)[0]
        configured = set(re.findall(r'^      ([A-Z_]+):', backend, re.MULTILINE))
        self.assertTrue(required)
        self.assertEqual(required - configured, set(), 'Missing required backend startup settings')


if __name__ == '__main__':
    unittest.main()
