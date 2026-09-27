"""Offline regression checks; these tests never contact a backend."""
import argparse
import gzip
import json
import unittest
from unittest.mock import patch

import benchmark_sentry as benchmark


class BenchmarkTests(unittest.TestCase):
    def test_dsn_never_embeds_key_in_request_url(self):
        url, key = benchmark.target("http://synthetic-key@localhost:9999/base/7")
        self.assertEqual(url, "http://localhost:9999/base/api/7/envelope/")
        self.assertEqual(key, "synthetic-key")
        with self.assertRaises(ValueError):
            benchmark.target("http://user:password@localhost/7")

    def test_workload_counts_retries_and_out_of_order_updates(self):
        captured = []

        class Response:
            status = 200

            def __init__(self, payload):
                self.payload = payload

            def __enter__(self):
                return self

            def __exit__(self, *args):
                return False

            def read(self, limit):
                return json.dumps(self.payload).encode()

        class Opener:
            def open(self, request, timeout):
                body = request.data
                if request.get_header("Content-encoding") == "gzip":
                    body = gzip.decompress(body)
                lines = body.splitlines()
                header = json.loads(lines[0])
                items = [(json.loads(lines[index])["type"], json.loads(lines[index + 1]))
                         for index in range(1, len(lines), 2)]
                captured.append((header, items))
                return Response({"id": header["event_id"]} if "event_id" in header else {"sessions": len(items)})

        args = argparse.Namespace(events=3, concurrency=2, retries=2, timeout=1,
                                  run_id="11111111-2222-4333-8444-555555555555", revision="test")
        with patch.dict("os.environ", {"CRASH_CACHE_DSN": "http://synthetic-secret@localhost:9999/7"}), patch.object(benchmark.urllib.request, "build_opener", return_value=Opener()):
            report = benchmark.run(args)
        self.assertEqual(report["summary"]["requests"], 14)
        self.assertEqual(report["summary"]["failed_or_invalid_acknowledgements"], 0)
        event_ids = [payload["event_id"] for _, items in captured for kind, payload in items if kind == "event"]
        self.assertEqual(len(set(event_ids)), 3)
        self.assertEqual(len(event_ids), 7)
        expected = report["expected_after_digest"]
        self.assertEqual(expected["reports"], 3)
        self.assertEqual(expected["issue_event_count"], 3)
        self.assertEqual([row["status"] for row in expected["session_rows"]], ["crashed", "exited", "ok"])
        self.assertNotIn("synthetic-secret", json.dumps(report))
        self.assertNotIn("localhost", json.dumps(report))
        mixed = [items for _, items in captured if len(items) == 2]
        self.assertEqual(len(mixed), 2)
        self.assertEqual(mixed[0][1][1]["seq"], 2)
        self.assertEqual(mixed[0][1][1]["status"], "crashed")


if __name__ == "__main__":
    unittest.main()
