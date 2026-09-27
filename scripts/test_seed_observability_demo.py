"""Contracts for deterministic, bounded and secret-free demo generation."""
import datetime as dt
import json
import struct
import unittest
import urllib.error
import uuid
from unittest.mock import patch
import zlib

from seed_observability_demo import demo_png, generate, retry_delay, send


def unpack(body):
    lines = body.split(b"\n", 1)
    header, remaining = json.loads(lines[0]), lines[1]
    items = []
    while remaining:
        line, remaining = remaining.split(b"\n", 1)
        metadata = json.loads(line)
        payload, remaining = remaining[:metadata["length"]], remaining[metadata["length"] + 1:]
        items.append((metadata, payload if metadata["type"] == "attachment" else json.loads(payload)))
    return header, items


class DemoTests(unittest.TestCase):
    def test_default_size_replay_and_original_binary_attachment(self):
        first, manifest = generate(dt.date(2026, 9, 1))
        self.assertEqual((first, manifest), generate(dt.date(2026, 9, 1)))
        self.assertTrue(1000 <= manifest["counts"]["events"] <= 3000)
        self.assertEqual(manifest["observed_installations"], 80)
        self.assertGreater(manifest["counts"]["attachments"], 0)
        self.assertGreater(manifest["counts"]["session_status_crashed"], 0)
        self.assertGreater(manifest["counts"]["session_status_ok"], 0)
        self.assertNotIn("dsn", json.dumps(manifest).lower())
        event_ids = set()
        updates = {}
        for envelope in first:
            header, items = unpack(envelope)
            for metadata, value in items:
                if metadata["type"] == "event":
                    self.assertEqual(header["event_id"], value["event_id"])
                    self.assertNotIn(value["event_id"], event_ids)
                    event_ids.add(value["event_id"])
                    self.assertIn(value["environment"], {"production", "staging"})
                    self.assertEqual(value["tags"]["synthetic"], "true")
                elif metadata["type"] == "session":
                    uuid.UUID(value["sid"])
                    uuid.UUID(value["did"])
                    updates.setdefault(value["sid"], []).append(value)
                elif metadata["type"] == "attachment":
                    self.assertEqual(value, demo_png())
                elif metadata["type"] == "log":
                    self.assertEqual(len(value["items"][0]["trace_id"]), 32)
        self.assertEqual(len(event_ids), manifest["counts"]["events"])
        for values in updates.values():
            self.assertEqual(values[0]["seq"], 0)
            self.assertTrue(values[0]["init"])
            if len(values) > 1:
                self.assertEqual(values[1]["seq"], 1)
                self.assertFalse(values[1]["init"])
                self.assertGreater(values[1]["duration"], 0)

    def test_v1_replay_bytes_remain_unchanged(self):
        _, manifest = generate(dt.date(2026, 9, 6), seed="demo-v1")
        self.assertEqual(manifest["sha256"], "5830d5cce2909ab9ea7a66d09497c919c3d963b43c43070a1abe0d4c546191ce")
        self.assertEqual(manifest["payload_bytes"], 1776064)

    def test_v2_events_include_exact_session_identity(self):
        envelopes, manifest = generate(dt.date(2026, 9, 6), days=1)
        self.assertEqual(manifest["seed"], "demo-v2")
        sessions = {}
        events = []
        for body in envelopes:
            for metadata, value in unpack(body)[1]:
                if metadata["type"] == "session":
                    sessions[value["sid"]] = value["did"]
                elif metadata["type"] == "event":
                    events.append(value)
        for value in events:
            self.assertEqual(sessions[value["tags"]["app_session_id"]], value["user"]["id"])

    def test_supplement_only_adds_deterministic_session_context_events(self):
        start = dt.date(2026, 9, 6)
        original, original_manifest = generate(start, seed="demo-v1")
        supplemental, manifest = generate(start, seed="demo-v1", session_context_only=True)
        self.assertEqual((supplemental, manifest), generate(start, seed="demo-v1", session_context_only=True))
        sessions, event_ids, contexts = {}, set(), {}
        for body in original:
            for metadata, value in unpack(body)[1]:
                if metadata["type"] == "session":
                    sessions[value["sid"]] = (value["did"], value["started"])
                elif metadata["type"] == "event":
                    event_ids.add(value["event_id"])
                    contexts[(value["user"]["id"], value["timestamp"])] = (value["platform"], value["contexts"]["os"])
        self.assertEqual(len(supplemental), original_manifest["counts"]["sessions"])
        self.assertEqual(manifest["counts"], {"activity_events": 875, "events": 875, "session_context_events": 875})
        new_ids = set()
        for body in supplemental:
            _, items = unpack(body)
            self.assertEqual(len(items), 1)
            metadata, value = items[0]
            self.assertEqual(metadata["type"], "event")
            self.assertEqual(value["level"], "info")
            self.assertNotIn("exception", value)
            self.assertEqual(value["tags"]["event_kind"], "app_session")
            self.assertNotIn(value["event_id"], event_ids | new_ids)
            new_ids.add(value["event_id"])
            self.assertEqual(sessions[value["tags"]["app_session_id"]], (value["user"]["id"], value["timestamp"]))
            self.assertEqual(contexts[(value["user"]["id"], value["timestamp"])], (value["platform"], value["contexts"]["os"]))

    def test_png_has_valid_chunk_checksums(self):
        png = demo_png()
        self.assertTrue(png.startswith(b"\x89PNG\r\n\x1a\n"))
        remaining = png[8:]
        while remaining:
            length = struct.unpack(">I", remaining[:4])[0]
            self.assertEqual(struct.unpack(">I", remaining[8 + length:12 + length])[0], zlib.crc32(remaining[4:8 + length]))
            remaining = remaining[12 + length:]

    def test_invalid_bounds_rejected(self):
        for kwargs in ({"days": 0}, {"installations": 101}, {"sessions_per_day": 101}, {"days": 90, "sessions_per_day": 100}, {"seed": ""}):
            with self.assertRaises(ValueError):
                generate(dt.date(2026, 9, 1), **kwargs)

    def test_release_regression_patterns_and_dates(self):
        envelopes, _ = generate(dt.date(2026, 9, 1))
        patterns = {}
        for body in envelopes:
            for metadata, event in unpack(body)[1]:
                if metadata["type"] == "event":
                    self.assertGreaterEqual(event["timestamp"], "2026-09-01")
                    self.assertLess(event["timestamp"], "2026-09-22")
                    if "exception" in event:
                        patterns.setdefault(event["release"], set()).add(event["tags"]["scenario"])
        self.assertIn("InventoryTimeout", patterns["synthetic-shop@1.0.0"])
        self.assertNotIn("InventoryTimeout", patterns["synthetic-shop@1.1.0"])
        self.assertIn("ImageDecodeError", patterns["synthetic-shop@1.1.0"])
        self.assertIn("CacheWriteFailure", patterns["synthetic-shop@1.1.1"])

    @patch("time.sleep")
    @patch("seed_observability_demo.urllib.request.build_opener")
    def test_rate_limit_retries_same_bytes_then_paces_next_envelope(self, opener, sleep):
        response = opener.return_value.open.return_value.__enter__.return_value
        response.status = 200
        opener.return_value.open.side_effect = [
            urllib.error.HTTPError("http://secret@host.invalid", 429, "rate limited", {"Retry-After": "2"}, None),
            opener.return_value.open.return_value, opener.return_value.open.return_value]
        self.assertEqual(send([b"one", b"two"], "http://secret@host.invalid/1", 1), 2)
        self.assertEqual([call.args[0].data for call in opener.return_value.open.call_args_list], [b"one", b"one", b"two"])
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [2.0, 0.05])

    @patch("time.sleep")
    @patch("seed_observability_demo.urllib.request.build_opener")
    def test_rate_limit_budget_and_nonretryable_status(self, opener, sleep):
        for status, retry in ((429, "120"), (403, "1")):
            opener.return_value.open.reset_mock()
            opener.return_value.open.side_effect = urllib.error.HTTPError("http://secret@host.invalid", status, "failed", {"Retry-After": retry}, None)
            with self.assertRaisesRegex(RuntimeError, f"HTTP {status}") as error:
                send([b"one"], "http://secret@host.invalid/1", 1)
            self.assertNotIn("secret", str(error.exception))
            self.assertEqual(opener.return_value.open.call_count, 1)
        sleep.assert_not_called()
        opener.return_value.open.reset_mock()
        opener.return_value.open.side_effect = lambda *args, **kwargs: (_ for _ in ()).throw(
            urllib.error.HTTPError("http://secret@host.invalid", 429, "failed", {}, None))
        with self.assertRaisesRegex(RuntimeError, "budget exhausted at envelope 1"):
            send([b"one"], "http://secret@host.invalid/1", 1)
        self.assertEqual(opener.return_value.open.call_count, 6)
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [1, 2, 4, 8, 16])

    @patch("seed_observability_demo.time.time", return_value=1790424000)
    def test_retry_after_date_and_malformed_fallback(self, _clock):
        self.assertEqual(retry_delay("Sat, 26 Sep 2026 12:00:05 GMT", 0), 5)
        self.assertEqual(retry_delay("invalid", 2), 4)
        self.assertEqual(retry_delay("0", 0), 0.05)

    @patch("seed_observability_demo.urllib.request.build_opener")
    def test_transport_failure_stops_without_leaking_dsn(self, opener):
        opener.return_value.open.side_effect = OSError("http://secret@host.invalid")
        with self.assertRaisesRegex(RuntimeError, "envelope 1") as error:
            send([b"one", b"two"], "http://secret@host.invalid/1", 1)
        self.assertNotIn("secret", str(error.exception))
        self.assertEqual(opener.return_value.open.call_count, 1)


if __name__ == "__main__":
    unittest.main()
