# BULL metrics ingestion

The metrics endpoint is the Sentry envelope endpoint. A metrics envelope contains exactly one `trace_metric` item, and cannot contain events, sessions, attachments, or other items. Unsupported or invalid metric envelopes are rejected with HTTP 422 before any metrics are stored. Existing event and session ingestion is unchanged.

The item has `content_type` `application/vnd.sentry.items.trace-metric+json`, an `item_count` matching the number of points, and a JSON payload with an `items` array. Each counter uses the Sentry metric fields `timestamp`, `trace_id`, `type`, `name`, `value`, and `attributes`. Attribute values use the Sentry typed form, for example `{"type":"integer","value":1}`. A random 32-hex-character `trace_id` is required for protocol compatibility but is not stored in the aggregates and must not be derived from an installation identifier.

Every point must include the string attributes `sentry.release` and `sentry.environment`. A release has the bounded form `MAJOR.MINOR.PATCH+BUILD`, with one to six ASCII digits per component; environment is `regtest`, `testnet`, or `mainnet`. Unknown attributes, including SDK-enriched user or device attributes, are rejected rather than stored. The BULL client uses a dedicated transport without automatic SDK enrichment.

| Name | Type and value | Additional attributes |
|---|---|---|
| `bull.installation.active` | `counter`, value `1` | None |

Timestamps are accepted only within the preceding 30 days or up to five minutes in the future. An entire batch is rejected if any point is invalid. Prototype payment, swap and migration counters are not accepted.

Metrics are aggregated by hour, project and allowed dimensions; metric envelopes are never written to `archive` or the digest queue. An identical replay is deduplicated transactionally by project and envelope-content digest. This does not deduplicate equivalent points in differently serialized envelopes. Metric aggregates and replay digests have a 30-day retention; expired payloads cannot recreate expired aggregates.

At most 1024 distinct aggregate series may exist for a project and UTC hour across all metric names and dimension combinations. A PostgreSQL transaction-level advisory lock serializes metric writes per project, then the server checks every affected hour before committing. A batch that would exceed the limit is rolled back in full, including its replay digest, and receives HTTP 422 with `Metric hourly series limit exceeded`. An identical replay or a new point for an existing series is accepted at capacity; a different hour has its own allowance. This limit bounds series per project-hour, not the number of projects or replay digests, and the per-project lock trades write concurrency for a strict bound.

The 30-day cleanup runs hourly alongside the analytics collector and is independent of `ANALYTICS_RETENTION_DAYS`. Hour buckets that overlap the retention boundary remain until their full hour has expired, plus at most one cleanup interval; replay digests expire after the newest point in the batch becomes too old. A replay is accepted only while every point is within the timestamp window. Cleanup failure is logged and retried on the next cycle; operators should monitor it.

## Metabase SQL

Connect using the existing read-only Metabase account. `metric_hour` contains no raw point, trace ID or envelope. `value_sum` is the delivered observation count for a series. Filter `project_id`, `environment`, and `release` explicitly to avoid mixing deployments. Sentinel `-1` or empty string marks a dimension not applicable to that metric.

### Daily active installations

Sum the hourly counters for each UTC date in the selected project and environment. The intended producer limits observations locally to one per opted-in installation per UTC day. This is a count of delivered observations: offline use or failed sends may be missing, and the server cannot deduplicate distinct envelopes from the same installation. A person using multiple installations can contribute multiple observations. The counter carries no installation or wallet identifier.

```sql
SELECT bucket_start::date AS utc_day, SUM(value_sum) AS observed_active_installations
FROM metric_hour
WHERE project_id = {{project_id}} AND environment = {{environment}}
  AND name = 'bull.installation.active'
GROUP BY 1
ORDER BY 1;
```

Deploy ingestion support before enabling the mobile producer. An ingestion version that does not recognize this metric rejects the whole batch.
