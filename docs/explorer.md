# Authenticated investigation with Metabase

Crash-cache stores Sentry-compatible reports; Metabase reads their PostgreSQL representation. A `report` or `issue` table is a storage interface, not the HTTP API expected by another error tracker's frontend. Metabase does not need that external API: its query builder can read related reporting views directly. The optional evidence endpoint described below supplies attachment bytes that SQL metadata alone cannot display.

The integration targets Metabase OSS **0.63.18.2**. The supplied profile budgets 2 GiB for the container with a 1 GiB Java heap, leaving room for native allocations and query processing; size larger deployments from their own workload. It uses authenticated questions, database metadata, relationships, detail views and dashboard links. It does not require a Python application backend, a custom dashboard application, an embedded analytics subscription or an API compatible with the Sentry web frontend. Crash-cache remains the Rust ingestion service; the Python scripts are operator provisioning and test tools.

## Investigation workflow

Start with a small overview of errors and affected identities, then select a problem. Its investigation should retain the project, environment, version and time selection. Open an individual report to read its message, ordered stack frames, source context and variables when supplied by the SDK. Continue to chronological breadcrumbs, correlated structured logs and available attachments. Device, system and release breakdowns help assess the scope of the problem; they should not displace the report itself.

Use query-builder questions over the reporting views for automatic **See these records**, **Break out by** and time zoom. Native SQL questions can filter their results and use explicit click destinations, but Metabase cannot rewrite their SQL for the full exploration menu. Public dashboard sharing lacks the authenticated exploration experience. Switching to authenticated access is therefore a functional improvement, not merely a login screen.

A saved question with `display: "object"` presents a report as a readable detail card. A table with a configured primary key also supports Metabase's record detail view at `/table/{table_id}/detail/{primary_key}`. Related foreign keys provide navigation between projects, issues, reports and their child records. JSON fields remain available for deeper inspection; arrays of frames and breadcrumbs have dedicated views because Metabase's JSON unfolding does not expand arrays.

## Overview and two focused workflows

**Overview** is the entry page: choose a named project and a period, inspect collection status, active installations, session starts and error impact, then compare the immediately preceding period of equal UTC duration. A zero previous value has no percentage; it is labelled newly observed or no previous observations. Changes describe recorded data and are not proof of improvement or regression. The project selector reads a maintained question over project names, so new projects appear without hard-coded IDs.

The **Explore this selection** navigation row carries the selected project and raw period value between Overview, Usage and Errors. Dashboard KPI links provide the same route. Native table-cell click actions are used because virtual Markdown/link-card substitution formats dates for humans and cannot reliably transmit a relative date token unchanged.

**Error reports** opens the Health overview dashboard. Its **Overview** tab answers how many error occurrences, grouped problems and affected identities were recorded, and when. **Problems** provides a ranked table: select a problem to open its occurrences, then select an event message or ID. Activity observations are excluded from that event list. **Event details** opens with a compact summary and the **Stack trace** tab; additional tabs contain **Breadcrumbs**, **Files**, and **Context**. A missing stack is explicitly reported rather than presented as a successful capture. Original frame source context remains in the metadata JSON; the first tab surfaces function, file/line and the source line; captured variables remain in Context.

**App usage** provides the audience workflow. Its native tabs separate the questions:

| Tab | Questions answered |
| --- | --- |
| Overview | Unique active installation IDs, first observed installations, sessions started, and their time series |
| Platforms | Active installations and session starts by operating system, plus audience trends by system |
| Devices | Active installations by system/version, device model and app version |
| Quality | Sessions still open, latest outcomes grouped by start period, and missing identities or system correlations |

Use **Period (UTC)** for relative shortcuts such as the previous seven days or an inclusive custom range, and **Group by** for `day`, `week` or `month` (or `hour` buckets within selected days). Each bucket recalculates distinct identities; weekly/monthly values are not sums of daily users. These are calendar UTC buckets, with Monday-start weeks, truncated to the selected interval. A partial first bucket can have a label before the selected start. Overview counters cover the entire selected interval regardless of bucket size. The comparison uses the immediately preceding interval with exactly the same UTC duration, independent of whether either interval contains telemetry.

The native date filter uses the read-only `calendar` view (2000-01-01 through 2100-12-31) to derive selection boundaries. Set the Metabase reporting timezone to UTC. `past7days` excludes the current day; the date picker offers current-period variants separately. Use continuous ranges for usage and comparison: an empty or disjoint selection produces an explicit status and suppresses scalar audience counters rather than claiming zero activity. The calendar bounds also bound open-ended selections; it is not an unbounded historical query. Old native audience links using `from`/`until` should be replaced with `period=YYYY-MM-DD~YYYY-MM-DD` or a Metabase relative token.

Headline activity and error figures appear first, followed by collection status and scoped navigation. This ordering prioritizes the two primary counters, but browser chrome and viewport height can leave the error count below the first screen on a physical phone. Detailed tables can still require scrolling. “No activity received” does not establish zero players; error-only data is identified as such. Missing identities, session-system matches and stack frames are detailed in Quality. Error problems are ordered by affected identities and then occurrences; technical IDs follow useful impact columns. Occurrence messages open the corresponding stack trace.

Audience comes only from explicit `app_session` and `app_activity` events, not from the population that suffered errors. The client must send a stable random installation identity consistently across its SDKs. First observed means the first retained activity for that identity within the project; it is not proof of installation time. Reinstalls, identity resets and retention affect that estimate. Store downloads, real people and concurrent players cannot be recovered from these records. Distinct per-version/device counts must not be summed: an installation can change version or device metadata within an interval.

Sentry session rows do not carry an operating-system dimension. Session charts correlate an activity observation only when **project, identity and canonical session UUID** all match (`tags.app_session_id` to the session `sid`). The observation supplies the OS name, not an inferred OS from an SDK platform such as `dart` or `native`. Missing/conflicting matches remain **Unknown** and are counted in Collection quality. This prevents plausible-looking but unsupported platform totals. Latest `ok` means still open in retained data, not currently connected or crash-free.

Primary navigation carries project and period, and switching tabs retains the filters. The issue-to-occurrence path additionally preserves environment and version; occurrence-to-evidence preserves project, event and originating period. Supplemental links to release/session investigations retain their own saved filters. These scopes are selections, not access-control boundaries.

## Version and upgrade decision

The initial dashboard measurements used **0.63.18**. The deployment now pins **0.63.18.2**, the stable hotfix image verified on September 27, 2026; **0.64.0-beta** remains a prerelease. The hotfixes correct row charts with remapped columns and custom action visibility in embedded editable dashboards; they do not introduce a new dashboard layout. The implementation uses native tabs, line/bar charts, tables, detail cards and explicit click destinations. Custom visualization plugins in the current documentation require Pro/Enterprise; they are not an OSS dependency of this workspace.

For a later upgrade, read the release notes and official upgrade guide for the target version, back up the **Metabase application database** separately from the crash-cache telemetry database, pin the image version, stop the old instance, and let the new instance migrate its metadata. Test login, query/filter mappings, tabs, click destinations and evidence access before accepting it. A rollback restores the pre-upgrade application database with its matching image; do not run an older image against already-migrated metadata. Use a persistent PostgreSQL application database for a maintained deployment. An H2 database in a disposable container is a laboratory setup, not production persistence.

## Install the reporting views

Apply normal crash-cache migrations first, including `20260926000001_rich_telemetry`. As the database/schema owner, apply:

```sh
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f docs/sql/investigation.sql
```

This creates ordinary views in `crash_cache_explorer`; it neither inserts telemetry nor decompresses archives. Grant the Metabase connection role only the required view access:

```sql
GRANT USAGE ON SCHEMA crash_cache_explorer TO metabase_readonly;
GRANT SELECT ON ALL TABLES IN SCHEMA crash_cache_explorer TO metabase_readonly;
```

The example role name follows the supplied observability deployment. Adjust it to the existing read-only role in another deployment. Reapply the grant when new views are added. Keep the underlying `project.public_key` and compressed archive payloads out of the Metabase data source permissions. Synchronize the source schema in Metabase after installing the views, then configure the entity keys and relationships.

| View | Purpose | Entity key and relationships |
| --- | --- | --- |
| `projects` | Project names without ingestion keys | `id` |
| `calendar` | Day dimension for relative/custom period boundaries | `day`; no telemetry |
| `issues` | Lifetime issue summary within one project | `id`; `project_id` → projects; `latest_report_id` → reports |
| `reports` | Event message, metadata, tags and contexts | `id`; `project_id` → projects; `issue_key` → issues |
| `frames` | One stack frame per row, including source and variables | `id`; `report_id` → reports |
| `breadcrumbs` | One preceding event per row | `id`; `report_id` → reports |
| `attachments` | Filenames, types, sizes and report associations | `id`; nullable `report_id` → reports |
| `logs` | Structured log records, including uncorrelated records | `id`; `project_id` → projects |
| `report_logs` | Exact trace relationship between logs and reports | Composite `(report_id, log_id)`; neither column is a unique key |
| `sessions` | Latest accepted state of each SDK session | `id`; `project_id` → projects |

In Metabase's table metadata, mark a unique entity key as `type/PK` and a relationship as `type/FK` with the target field selected. The REST equivalent is `PUT /api/field/{id}` with `semantic_type` and, for foreign keys, `fk_target_field_id`. Give fields human-readable names and descriptions. Do not declare a non-unique column as a primary key simply to obtain a clickable detail view.

The reporting views intentionally contain full messages, variables and custom contexts for authenticated staff. They are not a tenant security boundary. A dashboard project filter selects an analysis; it does not authorize access. Use database roles and Metabase data permissions for restrictions. A Metabase administrator has access to all connected projects.

## Provision the authenticated workspace

After schema synchronization, use the existing private Metabase administrator environment (`METABASE_URL`, `METABASE_ADMIN_EMAIL`, `METABASE_ADMIN_PASSWORD`) and run:

```sh
python3 scripts/provision_explorer.py --project 123 --database 2 --start 2026-09-01 --end 2026-09-21 --evidence-origin https://observability.example.com --event 456
```

Replace the project and Metabase source database IDs with those of the deployment. The `Crash-cache Explorer` collection contains **Health overview**, **Event investigation**, **Sessions and logs**, and **Releases and devices**, plus their reusable query-builder questions. The script also provisions **Overview**, **App usage**, and the project-choice/navigation questions. With `--evidence-origin`, an **Event details** dashboard presents raw context, frames, breadcrumbs, correlated logs and authenticated attachment links/previews. The origin must be the browser-facing Metabase origin with the evidence route proxied to crash-cache. `--event` selects its initial record; omit it to start empty, then select an event from investigation. `--evidence-origin same-origin` creates relative links, but inline image previews require an absolute HTTP(S) origin in Metabase 0.63.18.

The script configures table metadata and relationships, and prints the resulting collection, dashboard, question and table IDs. Open `/dashboard/{home.dashboard_id}` from the printed result, or `/collection/{collection_id}` on the authenticated Metabase origin. It does not create users or publish anonymous links. It updates only objects carrying its management marker and refuses to overwrite unrelated content with the same name.

The date-range filter includes both selected dates. Record tables initially show the latest 200 rows; open their question to change the limit. Project and period apply to all four dashboards. Environment/version filtering requires a corresponding field in the selected data: structured logs currently have no normalized environment/version columns. Session releases are SDK release identifiers and are not necessarily identical to report app-version labels. The mixed releases/devices page has distinct **Version** and **Release** filters, with their applicability shown in its header. Set `EXPLORER_DEMO=1` to label the workspace as synthetic. Read the dashboard descriptions before comparing those populations.

## Counting and correlation contracts

An issue's reporting key combines its project ID with its underlying issue ID. This prevents an issue shared by storage grouping from merging statistics across projects. Report IDs remain the database's unique record IDs. Child keys combine the report ID and its ordinal; do not convert these integer keys through a lossy client-side floating-point representation when constructing URLs.

`issues.event_count`, affected identities, first seen and last seen are **lifetime** values. To show a time/environment/version subset, filter `reports` first and then aggregate by `issue_key`. Filtering a lifetime issue's `last_seen` does not recalculate its count. Keep unknown identities visible separately. A supplied identity is not necessarily a person or an installation; its meaning depends on the SDK application's policy.

Views do not promise row order. Order reports by `event_at` and ID, frames by `position`, and breadcrumbs by `chronology`. The latter accounts for SDK timestamps and uses sequence to break ties. Structured log timestamps and report timestamps originate in seconds; stored breadcrumb timestamps originate in milliseconds and are converted in the view.

Logs are associated with a report only through a valid, nonzero matching trace ID **within the same project**. A trace can connect several reports to the same log. Do not sum joined rows as distinct logs without accounting for that multiplicity. Matching device names, nearby timestamps or similar messages does not establish a reliable relationship. Attachments join by project and event ID; an attachment can exist before its corresponding report is digested.

Sessions represent the latest accepted SDK session state, not one row per status update. Count logical sessions once. Keep open (`ok`), normally ended (`exited`), crashed and abnormal sessions distinguishable. An open session is not proof that the application completed without crashing; an abnormal exit is not a confirmed crash. Any crash-free percentage must state its denominator, time selection, eligible session states and collection coverage. Session IDs and user identities need not be shared between independent SDK integrations unless the client explicitly coordinates them.

Absence of recent reports does not prove an issue was resolved. An increase after a release is an association, not proof of a causal regression. Do not manufacture workflow states or production activity from demonstration data.

## Reading attachments

Attachment metadata is queryable, while payload bytes remain in compressed archives. The optional Rust endpoint retrieves one attachment by its metadata ID:

```text
GET /api/evidence/attachments/{id}
```

Enable it explicitly on the crash-cache server:

```dotenv
METABASE_AUTH_URL=http://metabase:3000/api/user/current
```

The value must be a fixed operator-configured HTTP(S) URL ending exactly in `/api/user/current`, without credentials, query parameters or fragments. The current reqwest dependency has no TLS backend enabled: use a trusted internal HTTP connection to Metabase with this build. An HTTPS URL fails closed; HTTPS upstream support requires an explicit TLS dependency configuration and verification. Browser-facing HTTPS can terminate at the common reverse proxy independently of this internal connection.

Route `/api/evidence/attachments/` from the **same browser origin as Metabase** to crash-cache, preserving the `Cookie` header. Other routes continue to Metabase. For example, an operator-managed reverse proxy can route the path prefix without exposing the backend's full API. Metabase table links or image URLs then point to the common browser origin plus `/api/evidence/attachments/{id}`. Do not embed passwords, ingestion keys or session tokens into SQL, dashboard parameters or attachment URLs.

The endpoint extracts only `metabase.SESSION` from cookies, or accepts an explicit `X-Metabase-Session` header; conflicting credentials are rejected. It forwards the selected token as a session header to the fixed authorization endpoint, without forwarding unrelated cookies. Only an **active Metabase administrator** is authorized. This deliberately grants all-project evidence access to administrators and is not a project-scoped end-user permission model. A public dashboard viewer cannot retrieve evidence through this endpoint.

Authorization refuses redirects, bounds the response to 64 KiB and applies a two-second request timeout. Four concurrent evidence requests are admitted. Database work and decompression run on a blocking worker, with bounded pool wait and SQL statement time; the existing compressed and uncompressed payload size settings apply. Extraction checks the joined archive's project, exact item index, attachment item type and payload size. When the option is absent, the route does not exist; authentication failures return 401/403 and unavailable authorization fails closed.

PNG and JPEG signatures select an inline image MIME type; these checks are **not full image validation**. Valid UTF-8 with a declared `text/plain` type can display as text. Other content, including SVG and HTML, downloads as `application/octet-stream`. The response uses a sanitized filename, `private, no-store`, `nosniff`, a restrictive sandbox Content Security Policy and no-referrer policy. Metabase can format an authenticated image URL as an image cell, or leave it as a link for opening the original. Do not replace this design with publicly accessible archive paths.

## Provide an authenticated account without email delivery

An existing administrator can create a user in **Admin → People**. For automation on the pinned version:

1. `POST /api/user` with the chosen `email`, `first_name` and `last_name`.
2. Only when administrator access is intended, `PUT /api/user/{id}` with `{"is_superuser": true}`.
3. `POST /api/user/{id}/password-reset-url` returns `password_reset_url`, valid for 48 hours.
4. Deliver that link privately to the intended user; they choose their password and then open the authenticated dashboard.

Configure Metabase's site URL to the reachable browser origin before generating links. The reset URL is a credential: keep it out of Git, screenshots, saved SQL and test logs. No SMTP configuration is required for this flow. Ordinary analysts can use query-builder exploration with appropriate data permissions without being administrators, but the optional evidence adapter currently restricts binary attachment access to administrators.

## Demonstration data and acceptance checks

`scripts/seed_observability_demo.py` generates original deterministic synthetic envelopes and a manifest. It defaults to a dry run and sends only with `--send`. Use a dedicated crash-cache project and stable start/seed parameters for reproducible replay. The default `demo-v2` events include their exact `app_session_id`; explicit `--seed demo-v1` preserves historical envelope bytes. To enrich an existing v1 demo with missing session context, keep its original start/seed/dimensions and add `--session-context-only --send`. This emits new deterministic informational observations only, without updating sessions or reproducing errors, logs or attachments. Replaying those observations retains their IDs. The workload contains several SDK platform labels, releases, sessions, failures, breadcrumbs, source context, variables, logs and small generated image attachments. The pictures are checkerboard fixtures, not application screenshots. No downloaded production dataset is necessary. `explorer_ux_fixtures.generate(scenario, date)` also provides bounded, original `empty`, `partial` and `volume` datasets. The partial sample deliberately omits identities, session context and one stack; the volume sample contains 350 errors across 300 distinct fingerprints. Ingest each in a separate synthetic project and verify its manifest before using it for UI acceptance.

After sending, compare database counts and session states with the manifest; HTTP acceptance only establishes admission, not successful digest processing. Keep the synthetic project clearly named in the interface. The data can demonstrate investigation and charts, but cannot establish production stability, real device distribution or ingestion capacity.

Acceptance should exercise an authenticated browser from overview to selected issue, report, stack frame, breadcrumb and attachment, preserving the intended filters. Check an ordinary analyst separately from an administrator. Verify anonymous and unauthorized evidence requests are rejected, image/text rendering works, and date/version filters change counts before aggregation. Inspect both narrow and desktop layouts. Unit tests and SQL query success alone do not prove that this interaction is intuitive.

## Scope relative to Sentry and GlitchTip

Sentry's issue detail interface combines impact, event history, representative events, stack traces, breadcrumbs, tags and attachments. GlitchTip's 2026 releases add structured logs and richer trace/span presentation. These are useful references for information architecture; this integration does not claim feature parity or reuse their application backend.

The implemented reporting surface covers stored reports, grouping, metadata, sessions, structured logs and attachment metadata, with optional authenticated attachment reading. Displayed source, variables, images and correlation depend on what SDKs actually send and what ingestion retains. It does not add a trace waterfall, distributed transaction explorer, session replay player, profiling interface, issue assignment, resolution/ignore history, regression state machine or alert delivery. Metabase's ordinary charts do not implement those domain behaviors by themselves.

Continue with Metabase while the desired work is filtering, comparison, relational investigation and reading individual evidence. If triage workflows become necessary, introduce explicit persisted state and authority checks before exposing write actions. Metabase actions can write PostgreSQL, but blindly updating telemetry tables would bypass those rules. A custom frontend becomes justified only by demonstrated interaction needs that remain awkward after this authenticated, relational approach; it is not required merely because crash-cache lacks Sentry's web API.

## Primary references

- [Metabase upgrade guide](https://www.metabase.com/docs/latest/installation-and-operation/upgrading-metabase)
- [Metabase v0.63.18 release](https://github.com/metabase/metabase/releases/tag/v0.63.18)
- [Native dashboard tabs](https://www.metabase.com/docs/latest/dashboards/introduction)
- [Custom visualizations and edition requirements](https://www.metabase.com/docs/latest/questions/visualizations/custom)

- [Metabase 0.63.18 drill-through contracts](https://github.com/metabase/metabase/blob/v0.63.18/docs/questions/visualizations/drill-through.md)
- [Metabase 0.63.18 detail visualization](https://github.com/metabase/metabase/blob/v0.63.18/docs/questions/visualizations/detail.md)
- [Metabase 0.63.18 JSON unfolding](https://github.com/metabase/metabase/blob/v0.63.18/docs/data-modeling/json-unfolding.md)
- [Metabase 0.63.18 field metadata API](https://github.com/metabase/metabase/blob/v0.63.18/src/metabase/warehouse_schema_rest/api/field.clj)
- [Metabase 0.63.18 user and password-reset API](https://github.com/metabase/metabase/blob/v0.63.18/src/metabase/users_rest/api.clj)
- [Metabase 0.63.18 session authentication](https://github.com/metabase/metabase/blob/v0.63.18/src/metabase/server/middleware/session.clj)
- [Metabase 0.63.18 actions and write access](https://github.com/metabase/metabase/blob/v0.63.18/docs/actions/introduction.md)
- [Sentry issue details](https://docs.sentry.io/product/issues/issue-details/)
- [Sentry issue states and triage](https://docs.sentry.io/product/issues/states-triage/)
- [GlitchTip 6.1, March 2026](https://glitchtip.com/blog/2026-03-23-glitchtip-6-1-released/)
- [GlitchTip 6.2, June 2026](https://glitchtip.com/blog/2026-06-22-glitchtip-6-2-released/)

The Metabase API and behavior references above are pinned to the deployed version. The Sentry documentation and GlitchTip release posts describe their respective products; they do not imply those capabilities are present in crash-cache.
