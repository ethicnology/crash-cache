# Metabase dashboards

The maintained dashboards belong to crash-cache and work against its normalized database. They do not depend on a game, engine or application repository. Configure the database connection, project ID and date range; use the same provisioner for mobile, desktop, web and backend Sentry clients. Available evidence depends on the SDK, its configuration and the ingestion formats supported by crash-cache. This is not a claim of complete Sentry protocol compatibility.

For authenticated exploration, per-event evidence and administrator attachment access, see [the Explorer workspace](explorer.md). It complements these aggregate dashboards and does not publish raw evidence anonymously.

## Views and evidence requirements

| View | Evidence used | Client requirements |
| --- | --- | --- |
| Overview | Session starts/outcomes, error volume, optional installation activity/device observations | Sessions for release health; the optional observation contract for population activity |
| Diagnostic quality | Breadcrumbs, stacks, variables, source context, custom contexts and symbolication status | Only fields actually sent and retained can appear |
| Collection | Archives, queue state, structured logs and attachment metadata | Supported envelope or multipart items; no application-specific tags required |
| Error investigation | Error timeline, components, exception classes and report summaries | Standard error reports; optional tags enrich classification |
| Data tables | Detailed versions of the overview metrics and potential duplicate captures | The optional occurrence identifier is required for duplicate-capture correlation |

The capture-layer dimension uses the optional `layer` tag, falling back to the standard Sentry event `platform` when the tag is absent or empty. Missing both is labeled `Unclassified`. The optional `component` tag accepts arbitrary application component names; no engine or component allowlist is imposed. Exception classes, breadcrumb categories and custom context family names are retained across SDKs. The investigation and breadcrumb-coverage views use the same layer fallback as the overview, so a Python client without custom layer tags can be selected consistently.

Custom classification names are metadata and can appear in shared dashboards. Raw exception messages, variable values, source text, breadcrumb contents and attachment bytes are not part of the summary views. Use technical, non-secret classification names. Project/date filters scope queries but are not a tenant authorization boundary; access is controlled through the operator's database and Metabase permissions. Public sharing is separately enabled by an explicit command.

## Optional client observations

Ordinary error reporting does not establish the population of healthy installations. Foreground activity/device panels use the documented `event_kind=app_session|app_activity` information-event convention. Any Sentry client can implement it using ordinary contexts and tags; it is not an automatically emitted Sentry SDK event. Without it, these panels are empty rather than guessing activity from errors. Session-start counts remain separately available when the client sends supported individual sessions.

The duplicate-capture table uses an optional shared `error_occurrence_id` tag for one failure propagated between components. Without this tag, event retry idempotence still works, but the dashboard cannot infer that two independently identified reports came from the same failure. See [sessions, observations and deduplication](observability.md) for exact semantics and the supported session subset.

## Provisioning

Follow the [deployment and provisioning instructions](../deploy/observability/README.md) for the pinned Metabase version, read-only database role and required environment variables. The Compose laboratory is optional: the provisioner also accepts an operator-managed Metabase instance and migrated crash-cache database.

Provision `OBSERVABILITY_VIEW=overview`, `diagnostics`, `pipeline`, `details` and `tables` in turn. Each view is parameterized by `CRASH_CACHE_PROJECT_ID`, `OBSERVABILITY_FROM` and `OBSERVABILITY_UNTIL`; the project selector can select another project accessible to the same database role. One provisioner manages one collection and database connection per Metabase instance. It does not create isolated per-tenant collections or enforce per-project access control.

The default notes describe generic project observations. Set `OBSERVABILITY_LABORATORY=1` for controlled test deployments; this changes labeling, not SQL or stored data. Pass the same setting when running `scripts/dashboard_navigation.py`. Navigation preserves project and period on configured chart drill-through; section links retain destination defaults. No application names, project IDs, credentials or public dashboard UUIDs are hard-coded into the provisioner.

## Verification

The generic-client regression uses a read-only PostgreSQL fixture with a Python report that has no custom layer tag. It failed before the platform fallback and passes afterward. Other SQL contracts cover custom components/classes/categories, project isolation, half-open dates, missing metadata, combined detail filters and direct distinct counts. These fixtures establish query semantics; they do not claim receipt from every possible Sentry SDK.
