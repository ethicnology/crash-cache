#!/usr/bin/env python3
"""Generate bounded, original synthetic Sentry data; dry-run unless --send is set.

Use a dedicated project and keep --start/--seed stable to replay the same envelopes.
No input files, production data or third-party datasets are used. HTTP acceptance
is not digest completion: verify the manifest against the database afterwards.
"""
from __future__ import annotations

import argparse
from collections import Counter
import datetime as dt
import hashlib
from email.utils import parsedate_to_datetime
import json
import os
import struct
import time
import urllib.error
import urllib.request
import uuid
import zlib

from benchmark_sentry import NoRedirect, compact, target, timestamp

NAMESPACE = uuid.UUID("7f5930e1-a19f-4eef-b5d7-6401e9585ecb")
PROFILES = (
    ("javascript", "browser", "Linux", "6.8", "Desktop browser"),
    ("python", "worker", "Linux", "6.8", "Compute node"),
    ("java", "mobile", "Android", "16", "Android demo phone"),
    ("cocoa", "mobile", "iOS", "19", "iOS demo phone"),
    ("native", "desktop", "Windows", "11", "Windows demo laptop"),
    ("dart", "desktop", "macOS", "15", "Mac demo laptop"),
)
PATTERNS = (
    ("InventoryTimeout", "inventory", "Inventory lookup exceeded its deadline"),
    ("SessionExpired", "identity", "Session refresh rejected an expired token"),
    ("ImageDecodeError", "media", "Thumbnail payload could not be decoded"),
    ("CacheWriteFailure", "storage", "Cache commit failed during synchronization"),
    ("ConnectionInterrupted", "transport", "Connection closed during a response"),
)


def demo_png() -> bytes:
    """An original 16x16 checkerboard, not an application screenshot."""
    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    pixels = b"".join(b"\0" + b"".join(bytes((28, 150, 145) if (x // 4 + y // 4) % 2 else (235, 244, 242)) for x in range(16)) for y in range(16))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 16, 16, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")


def encode_envelope(items: list[tuple[dict, object]], sent_at: dt.datetime) -> bytes:
    header = {"sent_at": timestamp(sent_at)}
    chunks = []
    for metadata, payload in items:
        if metadata["type"] == "event":
            header["event_id"] = payload["event_id"]
        encoded = payload if isinstance(payload, bytes) else compact(payload)
        chunks.extend((compact({**metadata, "length": len(encoded)}), encoded))
    return b"\n".join([compact(header), *chunks]) + b"\n"


def generate(start: dt.date, days: int = 21, installations: int = 80,
             sessions_per_day: int = 48, seed: str = "demo-v2",
             session_context_only: bool = False) -> tuple[list[bytes], dict]:
    if not (1 <= days <= 90 and 1 <= installations <= 100 and 1 <= sessions_per_day <= 100):
        raise ValueError("Bounds: days 1..90, installations 1..100, sessions-per-day 1..100")
    if days * sessions_per_day > 3000 or not seed or len(seed) > 80:
        raise ValueError("At most 3000 sessions; seed must contain 1..80 characters")
    namespace = uuid.uuid5(NAMESPACE, f"{seed}:{start}:{days}:{installations}:{sessions_per_day}")
    def identity(label: str) -> str:
        return str(uuid.uuid5(namespace, label))
    counts = Counter()
    patterns = Counter()
    releases = Counter()
    envelopes = []
    observed = set()
    for day in range(days):
        # The three release periods deliberately include an old fault disappearing,
        # a release-associated burst and a late regression. These are observations,
        # not fabricated issue workflow states (resolved/ignored).
        phase = min(2, day * 3 // days)
        version = ("1.0.0", "1.1.0", "1.1.1")[phase]
        release = f"synthetic-shop@{version}"
        daily_sessions = max(1, sessions_per_day * (75 + day * 17 % 26) // 100)
        for index in range(daily_sessions):
            number = day * sessions_per_day + index
            person = (day * 13 + index * 7) % installations
            observed.add(person)
            platform, layer, os_name, os_version, model = PROFILES[person % len(PROFILES)]
            when = dt.datetime.combine(start + dt.timedelta(days=day), dt.time(7), dt.timezone.utc) + dt.timedelta(seconds=index * 900 // sessions_per_day * 48)
            duration = 60 + (number * 37 % 1200)
            ended = when + dt.timedelta(seconds=duration)
            environment = "staging" if person % 10 == 0 else "production"
            session_id = identity(f"session:{number}")
            trace_id = identity(f"trace:{number}").replace("-", "")
            user_id = identity(f"installation:{person}")
            def event(kind: str, at: dt.datetime) -> dict:
                payload = {"event_id": identity(f"{kind}:{number}").replace("-", ""), "timestamp": timestamp(at),
                        "platform": platform, "release": release, "environment": environment, "level": "info",
                        "user": {"id": user_id}, "tags": {"synthetic": "true", "dataset": seed, "layer": layer,
                        "event_kind": "app_activity", "app_version": version},
                        "contexts": {"os": {"name": os_name, "version": os_version}, "device": {"model": model},
                                     "trace": {"trace_id": trace_id, "span_id": trace_id[:16], "op": "demo.checkout"}}}
                # Preserve explicitly requested v1 envelopes byte-for-byte for safe replay.
                if seed != "demo-v1" or session_context_only:
                    payload["tags"]["app_session_id"] = session_id
                return payload
            def session(seq: int, status: str, errors: int = 0) -> dict:
                return {"sid": session_id, "did": user_id, "seq": seq, "init": seq == 0,
                        "started": timestamp(when), "timestamp": timestamp(when if seq == 0 else ended),
                        "duration": 0 if seq == 0 else duration, "errors": errors, "status": status,
                        "attrs": {"release": release, "environment": environment}}
            if session_context_only:
                # Separate deterministic IDs enrich old demos without overwriting an
                # event, mutating session state or introducing artificial errors.
                context = event("session-context-v1", when)
                context["tags"]["event_kind"] = "app_session"
                context["message"] = "Synthetic session context supplement"
                envelopes.append(encode_envelope([({"type": "event"}, context)], when))
                counts.update(events=1, activity_events=1, session_context_events=1)
                continue
            activity = event("activity", when)
            activity["message"] = "Synthetic foreground observation"
            envelopes.append(encode_envelope([({"type": "event"}, activity), ({"type": "session"}, session(0, "ok"))], when))
            counts.update(events=1, activity_events=1, session_updates=1, sessions=1)
            releases[release] += 1
            # Stable integer selection keeps generation portable across Python versions.
            roll = int(hashlib.sha256(f"{namespace}:{number}".encode()).hexdigest()[:8], 16) % 100
            threshold = (24, 48, 19)[phase]
            fault = None
            if roll < threshold:
                fault = (0 if phase == 0 else 2 if phase == 1 else 3) if roll < threshold // 2 else (1 if roll % 2 else 4)
            crashed = fault == 3 or (fault is not None and number % 13 == 0)
            if fault is not None:
                error_type, component, message = PATTERNS[fault]
                error = event("error", ended - dt.timedelta(seconds=1))
                error.update(level="fatal" if crashed else "error", message=f"Synthetic demo: {message}",
                             fingerprint=["synthetic-demo", error_type, platform])
                error["tags"].update(event_kind="error", component=component, scenario=error_type)
                error["exception"] = {"values": [{"type": error_type, "value": message,
                    "mechanism": {"type": "synthetic", "handled": not crashed},
                    "stacktrace": {"frames": [{"filename": "demo/checkout.py", "function": "complete_checkout", "lineno": 18,
                        "in_app": True, "pre_context": ["def complete_checkout(order):", "    validate(order)"],
                        "context_line": "    persist(order)", "post_context": ["    return order.id"],
                        "vars": {"item_count": str(1 + person % 5), "retry": "1", "synthetic": "true"}},
                        {"filename": f"demo/{component}.py", "function": f"{component}_operation", "lineno": 42, "in_app": True}]}}]}
                error["breadcrumbs"] = {"values": [
                    {"timestamp": timestamp(when), "category": "navigation", "message": "Opened synthetic checkout", "level": "info"},
                    {"timestamp": timestamp(ended - dt.timedelta(seconds=2)), "category": "http", "message": "Synthetic service response", "level": "warning", "data": {"status_code": 503, "url": "https://service.invalid/demo"}}]}
                items = [({"type": "event"}, error), ({"type": "log", "item_count": 1}, {"items": [{
                    "timestamp": (ended - dt.timedelta(seconds=1)).timestamp(), "trace_id": trace_id, "span_id": trace_id[:16],
                    "level": "error", "body": f"Synthetic demo: {message}", "severity_number": 17,
                    "attributes": {"synthetic": {"type": "boolean", "value": True}, "component": {"type": "string", "value": component}}}]})]
                if number % 11 == 0:
                    items.append(({"type": "attachment", "filename": "synthetic-checkerboard.png", "content_type": "image/png", "attachment_type": "event.attachment"}, demo_png()))
                    counts["attachments"] += 1
                envelopes.append(encode_envelope(items, ended))
                counts.update(events=1, error_events=1, logs=1)
                patterns[error_type] += 1
            # Intentionally retain a few incomplete sessions, never count them healthy.
            status = "crashed" if crashed else "abnormal" if number % 53 == 0 else "ok" if number % 47 == 0 else "exited"
            counts[f"session_status_{status}"] += 1
            if status != "ok":
                envelopes.append(encode_envelope([({"type": "session"}, session(1, status, int(fault is not None)))], ended))
                counts["session_updates"] += 1
    manifest = {"schema_version": 1, "synthetic": True, "seed": seed,
                "from": start.isoformat(), "until": (start + dt.timedelta(days=days)).isoformat(),
                "counts": dict(sorted(counts.items())), "observed_installations": len(observed),
                "patterns": dict(patterns), "sessions_by_release": dict(releases), "envelopes": len(envelopes),
                "payload_bytes": sum(map(len, envelopes)),
                "sha256": hashlib.sha256(b"".join(envelopes)).hexdigest(),
                "limits": ["Synthetic observations, not production performance or population evidence.",
                           "Issue resolution is not inferred from absence of recent reports.",
                           "Open and abnormal sessions must remain visible.",
                           "HTTP acceptance does not establish successful digest processing."]}
    if session_context_only:
        manifest["session_context_only"] = True
    return envelopes, manifest


def retry_delay(header: str | None, attempt: int) -> float:
    """Honor both Retry-After forms; malformed/missing values use backoff."""
    if header:
        try:
            if header.strip().isdigit():
                return max(0.05, float(header.strip()))
            when = parsedate_to_datetime(header)
            if when.tzinfo is not None:
                return max(0.05, when.timestamp() - time.time())
        except (ValueError, TypeError, OverflowError):
            pass
    return float(2 ** attempt)


def send(envelopes: list[bytes], dsn: str, timeout: float, interval: float = 0.05) -> int:
    if not 0.05 <= interval <= 5 or not 0 < timeout <= 60:
        raise ValueError("interval must be 0.05..5 seconds and timeout greater than zero and at most 60 seconds")
    endpoint, key = target(dsn)
    opener = urllib.request.build_opener(NoRedirect())
    for index, body in enumerate(envelopes):
        if index:
            time.sleep(interval)
        request = urllib.request.Request(endpoint, data=body, method="POST", headers={
            "Content-Type": "application/x-sentry-envelope", "X-Sentry-Auth": f"Sentry sentry_version=7, sentry_key={key}"})
        started = time.monotonic()
        waited = 0.0
        for attempt in range(6):
            remaining = 60 - max(waited, time.monotonic() - started)
            if remaining <= 0:
                raise RuntimeError(f"HTTP 429 retry budget exhausted at envelope {index + 1}")
            try:
                with opener.open(request, timeout=min(timeout, remaining)) as response:
                    if response.status != 200:
                        raise RuntimeError(f"HTTP {response.status} at envelope {index + 1}")
                    response.read(65536)
                break
            except urllib.error.HTTPError as error:
                status = error.code
                delay = retry_delay(error.headers.get("Retry-After") if error.headers else None, attempt)
                error.close()
                remaining = 60 - max(waited, time.monotonic() - started)
                if status != 429:
                    raise RuntimeError(f"HTTP {status} at envelope {index + 1}; replay the unchanged dataset after diagnosis") from None
                if attempt == 5 or delay >= remaining:
                    raise RuntimeError(f"HTTP 429 retry budget exhausted at envelope {index + 1}; replay the unchanged dataset after the rate limit resets") from None
                time.sleep(delay)
                waited += delay
            except (urllib.error.URLError, OSError, ValueError):
                raise RuntimeError(f"Transport failure at envelope {index + 1}; replay the unchanged dataset after diagnosis") from None
    return len(envelopes)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--start", type=dt.date.fromisoformat, required=True, help="First synthetic UTC date, inclusive")
    parser.add_argument("--days", type=int, default=21)
    parser.add_argument("--installations", type=int, default=80)
    parser.add_argument("--sessions-per-day", type=int, default=48)
    parser.add_argument("--seed", default="demo-v2")
    parser.add_argument("--session-context-only", action="store_true", help="Only add new session context events; use the original seed/start/dimensions, including --seed demo-v1 for existing v1 data")
    parser.add_argument("--send", action="store_true", help="Explicitly POST to CRASH_CACHE_DSN; use an isolated synthetic project")
    parser.add_argument("--timeout", type=float, default=15)
    parser.add_argument("--interval", type=float, default=0.05, help="Minimum seconds between envelopes (0.05..5); HTTP 429 retries respect Retry-After within a 60-second budget")
    args = parser.parse_args()
    try:
        if not 0 < args.timeout <= 60:
            raise ValueError("timeout must be greater than zero and at most 60 seconds")
        envelopes, manifest = generate(args.start, args.days, args.installations, args.sessions_per_day, args.seed, args.session_context_only)
        manifest["http_accepted"] = send(envelopes, os.environ.get("CRASH_CACHE_DSN", ""), args.timeout, args.interval) if args.send else 0
        manifest["mode"] = "send" if args.send else "dry-run"
        print(json.dumps(manifest, indent=2, sort_keys=True))
    except (ValueError, RuntimeError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
