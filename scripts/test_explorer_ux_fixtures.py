"""Synthetic UX fixtures remain bounded and preserve intentional missing data."""
import datetime as dt
import unittest

from explorer_ux_fixtures import generate
from test_seed_observability_demo import unpack


class UxFixtureTests(unittest.TestCase):
    def test_replay_and_size_bounds(self):
        for scenario in ('empty', 'partial', 'volume'):
            first = generate(scenario, dt.date(2026, 9, 28))
            self.assertEqual(first, generate(scenario, dt.date(2026, 9, 28)))
            self.assertLessEqual(first[1]['envelopes'], 350)
            self.assertLess(first[1]['payload_bytes'], 600_000)
            ids = [value['event_id'] for body in first[0] for meta, value in unpack(body)[1] if meta['type'] == 'event']
            self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(generate('empty', dt.date(2026, 9, 28))[0], [])
        with self.assertRaises(ValueError):
            generate('invalid', dt.date(2026, 9, 28))

    def test_partial_data_is_deliberately_incomplete(self):
        envelopes, manifest = generate('partial', dt.date(2026, 9, 28))
        items = [item for body in envelopes for item in unpack(body)[1]]
        events = [value for meta, value in items if meta['type'] == 'event']
        activities = [value for value in events if value['tags']['event_kind'] == 'app_activity']
        sessions = [value for meta, value in items if meta['type'] == 'session']
        self.assertEqual(len(sessions), 3)
        self.assertEqual(sum('user' not in value for value in activities), 1)
        self.assertEqual(sum('app_session_id' not in value['tags'] for value in activities), 1)
        matched = [session for session in sessions if any(
            activity['tags'].get('app_session_id') == session['sid']
            and activity.get('user', {}).get('id') == session['did'] for activity in activities)]
        self.assertEqual(len(matched), 1)
        errors = [value for value in events if 'exception' in value]
        self.assertEqual(sum('stacktrace' not in value['exception']['values'][0] for value in errors), 1)
        self.assertEqual(manifest['expected']['sessions_without_platform_match'], 2)

    def test_volume_has_many_distinct_issues_and_different_impact(self):
        envelopes, _ = generate('volume', dt.date(2026, 9, 28))
        events = [unpack(body)[1][0][1] for body in envelopes]
        self.assertEqual(len(events), 350)
        self.assertEqual(len({tuple(value['fingerprint']) for value in events}), 300)
        self.assertEqual(len({value['user']['id'] for value in events}), 80)
        self.assertEqual(len({value['contexts']['os']['name'] for value in events}), 3)
        self.assertEqual(len({value['user']['id'] for value in events if value['fingerprint'][-1] == 'operation-0'}), 2)
        self.assertTrue(all(value['tags']['synthetic'] == 'true' for value in events))


if __name__ == '__main__':
    unittest.main()
