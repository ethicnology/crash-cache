#!/usr/bin/env python3
"""Bounded Sentry ingestion compatibility and HTTP latency benchmark.

Use a disposable project. CRASH_CACHE_DSN is read only from the environment and
never included in the report. This measures backend ingestion acknowledgements,
not digest completion, game frame rate, or device SDK overhead. The operator must
verify expected database counts after the digest queue drains.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import datetime as dt
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, new_url):
        return None


def compact(value: object) -> bytes:
    return json.dumps(value, separators=(",", ":"), sort_keys=True).encode("utf-8")


def timestamp(value: dt.datetime) -> str:
    return value.isoformat(timespec="milliseconds").replace("+00:00", "Z")


def target(dsn: str) -> tuple[str, str]:
    try:
        parsed = urllib.parse.urlsplit(dsn)
        key = parsed.username
        parts = parsed.path.rstrip("/").rsplit("/", 1)
        project = parts[-1]
        if parsed.scheme not in {"http", "https"} or not parsed.hostname or not key or not project.isdecimal():
            raise ValueError
        if parsed.query or parsed.fragment or parsed.password:
            raise ValueError
        hostname = parsed.hostname
        host = f"[{hostname}]" if ":" in hostname else hostname
        if parsed.port:
            host += f":{parsed.port}"
        prefix = parts[0] if len(parts) == 2 else ""
        url = urllib.parse.urlunsplit((parsed.scheme, host, f"{prefix}/api/{project}/envelope/", "", ""))
        return url, key
    except (ValueError, TypeError):
        raise ValueError("CRASH_CACHE_DSN must be a valid Sentry project DSN") from None


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return round(ordered[max(0, math.ceil(fraction * len(ordered)) - 1)], 3)


def run(args: argparse.Namespace) -> dict:
    endpoint, key = target(os.environ.get("CRASH_CACHE_DSN", ""))
    run_id = str(uuid.UUID(args.run_id)) if args.run_id else str(uuid.uuid4())
    namespace = uuid.UUID(run_id)
    now = dt.datetime.now(dt.timezone.utc)
    event_ids = [uuid.uuid5(namespace, f"event-{index}").hex for index in range(args.events)]
    session_ids = [str(uuid.uuid5(namespace, f"session-{index}")) for index in range(3)]
    installations = [str(uuid.uuid5(namespace, f"installation-{index}")) for index in range(3)]
    release = f"crash-cache-benchmark@1.0.0+{run_id}"
    records: list[dict] = []

    def session(index: int, sequence: int, status: str) -> dict:
        return {
            "sid": session_ids[index], "did": installations[index], "seq": sequence,
            "init": sequence == 0, "started": timestamp(now),
            "timestamp": timestamp(now + dt.timedelta(seconds=sequence)),
            "duration": float(sequence), "errors": 1 if status == "crashed" else 0,
            "status": status, "attrs": {"release": release, "environment": "benchmark"},
        }

    def event(index: int) -> dict:
        return {
            "event_id": event_ids[index], "timestamp": timestamp(now),
            "platform": "other", "level": "error", "release": release,
            "environment": "benchmark", "message": "Synthetic ingestion benchmark error",
            "fingerprint": ["crash-cache-benchmark", run_id],
            "user": {"id": installations[0]},
            "tags": {"benchmark_run": run_id, "component": "synthetic", "layer": "harness"},
        }

    def send(label: str, items: list[tuple[str, dict]], attempt: int = 0) -> dict:
        header = {"sent_at": timestamp(now + dt.timedelta(milliseconds=attempt))}
        for kind, payload in items:
            if kind == "event":
                header["event_id"] = payload["event_id"]
        chunks = [compact(header)]
        for kind, payload in items:
            encoded = compact(payload)
            chunks.extend((compact({"type": kind, "length": len(encoded)}), encoded))
        raw = b"\n".join(chunks) + b"\n"
        compressed = attempt % 2 == 1
        body = gzip.compress(raw, mtime=attempt) if compressed else raw
        headers = {"Content-Type": "application/x-sentry-envelope", "X-Sentry-Auth": f"Sentry sentry_version=7, sentry_key={key}"}
        if compressed:
            headers["Content-Encoding"] = "gzip"
        request = urllib.request.Request(endpoint, data=body, headers=headers, method="POST")
        started = time.perf_counter()
        status = None
        response_bytes = 0
        error = None
        valid_ack = False
        try:
            with urllib.request.build_opener(NoRedirect()).open(request, timeout=args.timeout) as response:
                status = response.status
                content = response.read(65537)
                response_bytes = len(content)
                if len(content) <= 65536:
                    acknowledgement = json.loads(content)
                    valid_ack = acknowledgement.get("id") == header["event_id"] if "event_id" in header else acknowledgement.get("sessions") == len(items)
        except urllib.error.HTTPError as failure:
            status = failure.code
            error = "http_error"
        except (urllib.error.URLError, TimeoutError, OSError):
            error = "transport_error"
        except (ValueError, AttributeError):
            error = "invalid_acknowledgement"
        return {"case": label, "gzip": compressed, "status": status,
                "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
                "request_bytes": len(body), "uncompressed_bytes": len(raw),
                "response_bytes": response_bytes, "valid_acknowledgement": valid_ack, "error": error}

    started = time.perf_counter()
    for index in range(3):
        records.append(send("session_init", [("session", session(index, 0, "ok"))]))
    batch_started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.concurrency) as executor:
        futures = [executor.submit(send, "distinct_event", [("event", event(index))], index) for index in range(args.events)]
        records.extend(future.result() for future in futures)
    batch_seconds = time.perf_counter() - batch_started
    for attempt in range(1, args.retries + 1):
        records.append(send("same_event_retry", [("event", event(0))], attempt + args.events))
    # A duplicate event carries a newer session update. Its event savepoint must
    # not roll back that session update or increment the issue occurrence count.
    records.append(send("mixed_duplicate_and_crash", [("event", event(0)), ("session", session(0, 2, "crashed"))], 201))
    records.append(send("stale_session_after_crash", [("session", session(0, 1, "ok"))], 202))
    records.append(send("mixed_duplicate_retry", [("event", event(0)), ("session", session(0, 2, "crashed"))], 203))
    records.append(send("session_exit", [("session", session(1, 2, "exited"))], 204))
    records.append(send("stale_session_after_exit", [("session", session(1, 1, "ok"))], 205))
    records.append(send("session_exit_retry", [("session", session(1, 2, "exited"))], 206))
    elapsed = time.perf_counter() - started
    durations = [record["elapsed_ms"] for record in records]
    event_records = [record for record in records if record["case"] == "distinct_event"]
    event_durations = [record["elapsed_ms"] for record in event_records]
    acknowledged_events = sum(record["status"] == 200 and record["valid_acknowledgement"] for record in event_records)
    status_counts: dict[str, int] = {}
    for record in records:
        status = str(record["status"]) if record["status"] is not None else "transport_error"
        status_counts[status] = status_counts.get(status, 0) + 1
    failures = sum(not (record["status"] == 200 and record["valid_acknowledgement"]) for record in records)
    return {
        "schema_version": 1, "measurement_kind": "backend_ingestion_http",
        "run_id": run_id, "captured_at": timestamp(now),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "backend_revision": args.revision,
        "workload": {"distinct_events": args.events, "event_retries": args.retries,
                     "concurrency": args.concurrency, "timeout_seconds": args.timeout,
                     "warmup_requests": 3, "warmup_kind": "session_init", "repetitions": 1},
        "summary": {"requests": len(records), "failed_or_invalid_acknowledgements": failures,
                    "http_status_counts": status_counts, "elapsed_seconds": round(elapsed, 3),
                    "distinct_event_batch_seconds": round(batch_seconds, 3),
                    "distinct_event_acknowledgements_per_second": round(acknowledged_events / batch_seconds, 3),
                    "distinct_event_latency_ms": {"p50": percentile(event_durations, .5), "p95": percentile(event_durations, .95),
                                                   "p99": percentile(event_durations, .99), "max": max(event_durations)},
                    "latency_ms": {"p50": percentile(durations, .5), "p95": percentile(durations, .95),
                                   "p99": percentile(durations, .99), "max": max(durations)},
                    "request_bytes": sum(record["request_bytes"] for record in records),
                    "uncompressed_bytes": sum(record["uncompressed_bytes"] for record in records)},
        "expected_after_digest": {"verification_status": "not_verified",
            "release": release, "benchmark_run_tag": run_id, "reports": args.events,
            "issues": 1, "issue_event_count": args.events, "sessions": 3,
            "unique_session_installations": 3, "queue_errors_for_run": 0,
            "event_ids": event_ids,
            "session_rows": [{"sid": sid, "distinct_id_hash": hashlib.sha256(installations[index].encode()).hexdigest(),
                              "sequence": "2" if index < 2 else "0",
                              "status": ["crashed", "exited", "ok"][index]} for index, sid in enumerate(session_ids)]},
        "limitations": ["HTTP acknowledgement is not completed ingestion or symbolication.",
                        "Expected database state must be verified separately after queue drain.",
                        "Synthetic backend workload; not game FPS, smartphone SDK overhead, or production capacity.",
                        "Latencies include the caller network and scheduler; record host/container conditions separately.",
                        "Only synthetic installation identifiers are generated; no device identifiers are collected."],
        "requests": records,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--events", type=int, default=100)
    parser.add_argument("--concurrency", type=int, default=4)
    parser.add_argument("--retries", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=10)
    parser.add_argument("--run-id", help="Optional UUID for reproducibility; use a fresh ID for independent runs")
    parser.add_argument("--revision", help="Backend commit identity supplied by the operator")
    parser.add_argument("--output", type=Path, required=True, help="Sanitized JSON report destination")
    args = parser.parse_args()
    if not (1 <= args.events <= 1000 and 1 <= args.concurrency <= 8 and 1 <= args.retries <= 20 and 0 < args.timeout <= 30):
        parser.error("Bounds: events 1..1000, concurrency 1..8, retries 1..20, timeout (0,30]")
    try:
        report = run(args)
    except ValueError:
        parser.error("Invalid DSN or run UUID; sensitive input values are not displayed")
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"run_id": report["run_id"], "summary": report["summary"],
                      "database_verification": "pending"}, indent=2))
    return 1 if report["summary"]["failed_or_invalid_acknowledgements"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
