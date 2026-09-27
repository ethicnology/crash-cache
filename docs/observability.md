# Application sessions and diagnostics

The supported individual session fields follow the [Sentry session protocol](https://develop.sentry.dev/sdk/telemetry/sessions/) (version 1.7.1, reviewed September 26, 2026). The protocol is a release-health transport, not a player-account system. Use one session owner per application process; disable independent automatic tracking in other embedded SDKs. Renderer handoffs do not start or end application sessions.

## Identity and ordering

Generate one random installation identifier, persist it locally outside cloud backup, and supply the same value as session `did` and event `user.id`. Do not use advertising, hardware, network, account, or invitation identifiers. Clearing application storage creates a new installation identity. A single person with multiple installations is counted multiple times. A shared installation is counted once. This identifier is pseudonymous, not anonymous.

The normalized session and event user dimensions store a SHA-256 digest of that identifier, allowing SQL correlation without retaining its supplied value there. Archived envelopes still contain the original payload: hashing normalized columns is not a raw-archive privacy or retention policy. Production retention, deletion, access control and consent remain application/operator responsibilities.

A session is unique within a project by its UUID `sid`. Retries keep this ID. Session state changes carry an increasing `seq`; the initial update forces sequence zero. Missing sequence uses the update timestamp in milliseconds. The complete unsigned 64-bit sequence is stored as decimal text and compared numerically. An atomic conflict update retains the state with the highest sequence and preserves installation identity, start, release and environment from the first received state. Receiving a delayed initial update records that initialization was seen without reopening a later crashed session. Duplicate sequence numbers do not replace state. The client must provide a complete state and must not change immutable attributes; sessions with missing identity cannot later be upgraded by changing `did`.

## Existing database upgrade

The additive migration reconstructs timestamp-based sequence values for previously stored non-initial sessions so that a delayed old update cannot replace their latest state. Initial rows retain sequence zero. Historical explicit SDK sequence values were not stored by the old schema and cannot be recovered from session-only ingestion; avoid assuming recovery of that unavailable information. If a legacy update timestamp is malformed, migration stops with the affected row ID and a repair instruction. Review and repair that row deliberately, then retry; the migration does not delete the session or silently mark it healthy. Run the migration transactionally, as the normal startup migration runner does. Downgrade refuses to restore global event-ID uniqueness when two projects legitimately contain the same event ID; it does not discard either report.

## Healthy device observations

Sessions do not support arbitrary device or OS attributes. Send one standard Sentry information event at each application-session start, including healthy starts, from the same owner:

```json
{
  "event_id": "ba67310d89ca44b8adf8a04b626446fa",
  "timestamp": "2026-09-26T10:00:00Z",
  "level": "info",
  "message": "app.session.started",
  "release": "application@1.0.0",
  "environment": "test",
  "user": {"id": "random-installation-id"},
  "tags": {
    "event_kind": "app_session",
    "app_session_id": "7c7b6585-f901-4351-bf8d-02711b721929",
    "layer": "client",
    "component": "application"
  },
  "contexts": {
    "os": {"name": "Android", "version": "16"},
    "device": {"model": "Pixel 6a"},
    "app": {"app_version": "1.0.0", "app_build": "123"}
  }
}
```

Keep the same `event_id` when retrying this observation. The session tag may use the SDK's 32-hex or hyphenated UUID representation; the dashboard validates either form and compares their normalized values. The server stores normal Sentry contexts and tags; it does not need a private session-attribute extension. This observation must not contain an exception or increment session error counts. If sampling observations, mark dashboards as sampled: an error-only device distribution is not the distribution of all installations. Session-only ingestion does not create a raw archive; event-bearing envelopes do.

## Foreground activity observations

To measure active installations beyond session starts, the application owner may send the same standard information event with `event_kind=app_activity` and message `app.foreground.active`. Emit one observation on foreground resume and at intervals no greater than 60 seconds while the application is foregrounded. For applications embedding multiple renderers or SDKs, one owner covers the whole process; hiding an embedded view does not imply that the application is backgrounded. Stop the periodic observation when the application actually leaves the foreground. Include the current session UUID, installation identity and technical contexts exactly as for the start observation, without position, map content, invitation, network or input data.

Generate one event UUID when creating each observation and preserve that UUID across offline retries. An offline queue must retain the original observation timestamp, not replace it with upload time. A missed observation is missing evidence, not proof of inactivity. This heartbeat contract is a client integration requirement; the SQL and backend alone do not implement its timer or lifecycle ownership. Verify actual event cadence and background cessation on devices before describing the population metric as measured.

The activity query counts distinct installations with either a start or foreground observation within each UTC calendar period. It counts an installation only once per period even if it emits many observations, opens multiple sessions or changes device dimensions. A short foreground visit spanning a UTC day boundary can still miss the next day when no observation occurs after midnight; the cadence bounds observation spacing during continued foreground operation but does not yield exact playtime or concurrency. Do not infer a full minute of activity from each event.

## Dashboards

The [generic Metabase guide](metabase.md) distinguishes standard SDK evidence from optional client observations and explains configuration for different applications.

[The read-only SQL queries](sql/observability.sql) cover session starts and observed foreground installations by UTC day, ISO week and month; release health and ended-session durations; observed technical dimensions; errors by layer/component; and potential duplicate captures of the same logical error. Run them with `psql` and an operator-supplied project ID and half-open time range as shown in the file. For Metabase, replace the psql variables with typed dashboard parameters and run each SELECT as a separate question. Partial weeks and months at the range boundaries remain partial.

Installation counts describe installations that started a recorded session in the period. They are not people, concurrent players, playtime, or necessarily all active installations: a session spanning a day boundary is not counted as a new start on the next day. Count distinct IDs over the whole target period; never add daily unique counts to obtain weekly or monthly uniques. Use the separate foreground-activity query for observations across long sessions. Do not silently relabel start counts as that stronger metric.

`percent_without_reported_crash` includes open and abnormal sessions in its denominator and is not evidence that every session finished successfully. Inspect `still_open`, `abnormal` and missing identities alongside it. Session updates lost when a process dies or remains offline can delay or prevent accurate health classification. The duration average only includes cleanly exited sessions, matching the Sentry protocol.

Technical dimensions use observations from healthy and unhealthy starts and foreground activity. Correlation requires matching project, session UUID and hashed installation ID. The comparison accepts only valid 32-hex or hyphenated UUID spellings, ignores hexadecimal case and hyphen placement between those two supported forms, and leaves the persisted tag unchanged. A late session update can temporarily leave an observation uncorrelated. The [read-only UUID correlation regression](sql/session-correlation-check.sql) must return zero failure rows; it covers both UUID forms, uppercase hex, malformed input, missing identity and mismatched project/installation. An installation that upgrades its OS or app during the selected period can occur in multiple dimension groups; these group counts must not be summed to obtain total unique installations.

## Deduplication and scope

Event occurrence identity is `(project_id, event_id)`, independent of issue fingerprint. A retried event must not increase issue occurrence counts. Different event IDs remain separate occurrences even when grouped into one issue. To detect a failure propagated across layers, instrument those boundaries with a shared non-secret `error_occurrence_id` and choose one capture owner. The duplicate-capture query diagnoses repeated occurrences carrying that explicit identifier; SDKs do not automatically supply it, and an empty query result cannot prove the absence of uncorrelated duplicates. Custom Sentry fingerprints control grouping, not deduplication. Native crash symbolication and SDK crash-handler coordination are separate capabilities to validate.

Individual mobile sessions with UUIDs are supported. Session aggregates (`sessions` items), SID-less exited sessions, official Sentry's five-day update-retention behavior and the hosted Sentry release-health API are not implemented by this change. Do not describe this subset as complete Sentry backend parity. Session `ip_address` and `user_agent` are intentionally not persisted as normalized dimensions, consistent with their protocol filtering purpose. No device or system information is inferred from a session payload.

## Recorded validation

The [September 26, 2026 validation report](observability-validation.md) records bounded HTTP measurements, separately verified persisted counts, Metabase provisioning, SQL permissions and UUID correlation evidence with their limitations.
