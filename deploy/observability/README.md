# Observability deployment and dashboards

The [generic dashboard guide](../../docs/metabase.md) describes reusable views for any supported Sentry client and their optional evidence requirements. The dashboard provisioner is not tied to this laboratory deployment.

This opt-in Compose deployment isolates the compatibility laboratory from the existing root Compose stack. It runs PostgreSQL, crash-cache, Symbolicator and a private HTTP symbol source. The additional `dashboards` profile runs Metabase and a Caddy proxy that serves dashboards and authenticated attachments from one origin. Do not point it at an existing deployment's database volume. There are no fixed container names, so a distinct Compose project name isolates networks and volumes.

The definitions use the Compose specification and can be run with Docker Compose or `podman compose` with a compatible provider. Linux can run Podman directly; macOS needs an existing running Podman machine with sufficient memory and disk. Provisioning dashboards uses only Python 3's standard library and runs on both systems. Neither Compose syntax nor this script establishes that a particular host/runtime combination has been tested.

## Configure and start

From the repository root, copy `deploy/observability/.env.example` to `deploy/observability/.env`, restrict it to the operator, and populate the four independent passwords. Generate hexadecimal values with `openssl rand -hex 32`; using hex also avoids URI-encoding ambiguity in the application database URL. The populated `.env` is ignored by Git. Do not publish expanded `compose config` output, container inspection output or environment dumps: they contain credentials.

Set `SYMBOLS_DIRECTORY` to an existing absolute directory holding matching build artifacts and `SYMBOLICATOR_SOURCES_FILE` to an existing absolute private JSON file copied from `sources.example.json`. The example uses an internal HTTP source and the `native` layout. For ELF, place matching debug files at `.build-id/<first-two-build-id-hex>/<remaining-build-id-hex>.debug` under the symbols directory. The source URL includes `.build-id/` because Symbolicator appends only the remaining build-ID path. Keep symbols readable by the unprivileged static-file container. Never substitute symbols from a different build. Adding an image or library without its matching symbols does not establish symbolication.

```sh
podman compose --env-file deploy/observability/.env \
  -f deploy/observability/compose.yml -p crash-cache-observability \
  --profile observability up -d --build
```

Add `--profile dashboards` before `up` to include Metabase and its proxy. Set `METABASE_SITE_URL` to the browser-facing origin; its default is `http://localhost:3002`, matching `METABASE_PROXY_PORT=3002`. Use that proxy port for the browser and `METABASE_URL`, not the direct Metabase debugging port (`METABASE_LAB_PORT`, default 3001). The Compose provider must support profiles, conditional `depends_on`, environment-variable requirements and resource limits. On an SELinux-enforcing Linux host, configure appropriate labels for these dedicated bind mounts; do not disable SELinux globally or relabel unrelated shared directories.

The ingestion endpoint and dashboard are published only on loopback. PostgreSQL, Symbolicator and the symbol HTTP server have no published host ports. Access a remote laboratory through a deliberate authenticated tunnel; do not expose an unauthenticated symbol server or dashboard setup endpoint publicly. Symbolicator allows private-address sources so it can reach the internal static server; source definitions are operator-owned, never supplied by events. This relaxes Symbolicator's destination restriction only within this trusted laboratory deployment: keep its API reachable exclusively by crash-cache on the private network and retain operator-only control of the sources file. Without `connect_to_reserved_ips: true`, a correct build-ID path can still fail with `destination is restricted`; that is a source-access failure, not proof of missing symbols. After correcting an access failure, account for cached download failures when repeating the verification.

The database initialization script runs only on the first initialization of the dedicated volume. It creates a migration-capable application role, a separate Metabase application-database role and a SELECT-only analytics role with default privileges on future application tables. Changing passwords in `.env` does not rotate existing database roles. Rotate credentials deliberately rather than deleting the volume to make startup succeed.

The configured memory ceilings total approximately 3.7 GiB without dashboards and 5.8 GiB with dashboards; these are limits, not measured usage or minimum host requirements. Build memory is separate. The application payload ceilings are laboratory settings suitable for native dump testing, not a proven production sizing recommendation.

## Fresh containers and persistence

No existing Metabase container, H2 file or manually configured proxy is required. PostgreSQL's `postgres_data` named volume holds two separate databases: crash-cache telemetry and Metabase accounts/questions/dashboards. The Metabase container is disposable; its application metadata uses `MB_DB_TYPE=postgres`. Keep the same project name, passwords and volume when replacing containers. The initialization SQL runs only on an empty volume; application migrations run at backend startup. A new, empty volume creates an empty installation, after which the provisioning steps below reconstruct the maintained workspace. It does not restore previous users, telemetry or manual changes.

The Caddy configuration routes only `/api/evidence/attachments/*` to crash-cache and all other paths to Metabase, preserving session cookies and attachment paths. The backend's fixed internal authorization URL defaults to `http://metabase:3000/api/user/current`; binary evidence still requires an active Metabase administrator. No public sharing or authentication bypass is enabled. Caddy's admin API and request access log are disabled; reset links and session tokens must not enter access logs. This is an internal HTTP listener bound to host loopback, not automatic public TLS. Terminate external HTTPS or use an authenticated tunnel deliberately. Set the public origin consistently in `METABASE_SITE_URL` and `--evidence-origin`.

For container replacement, run `down` without `--volumes`, then the same `up -d` command. Verify login, the same dashboard IDs, a controlled report and an authenticated attachment after replacement. Back up both databases separately; volume persistence is not a backup. Never use a different PostgreSQL major image against an existing data directory without its documented migration procedure.

## Create a project and configure client DSNs

Inside the configured crash-cache container, create a project with a positional name:

```sh
podman compose --env-file deploy/observability/.env \
  -f deploy/observability/compose.yml --profile observability \
  exec crash-cache crash-cache project create my-application
```

Use the same Compose project name and environment as the deployment; if it was started with `-p`, include that same option here. On a directly installed server with its database environment loaded, the equivalent command is `crash-cache project create my-application`. `crash-cache project create --help` documents `[NAME]` and optional `--key`; there is no `--name` flag. The command prints the project ID and an example DSN. Avoid creating another project merely to recover its configuration: `crash-cache project list` lists existing project keys and IDs in the operator terminal.

A client DSN has the form `https://PUBLIC_KEY@errors.example.com/PROJECT_ID`. Preserve the issued key and project ID, but replace the printed scheme, hostname and port with the ingestion address reachable from that client. The current CLI builds its example from the configured bind address and always prints `http://`; it cannot infer a public TLS proxy, forwarded host port or external hostname. In particular, `0.0.0.0` is a listen address, and a container service name is generally not reachable from a phone.

| Address | Purpose | Consumer |
| --- | --- | --- |
| `CRASH_CACHE_HOST` / `CRASH_CACHE_PORT` | Server listen address inside its runtime | crash-cache process |
| `CRASH_CACHE_LAB_PORT` | Loopback host port mapped to the container | Local tests or an explicitly configured proxy/tunnel |
| Public HTTPS ingestion origin | Externally reachable address, with forwarding to crash-cache | Sentry SDK DSN |
| `METABASE_URL` | Dashboard and administration API | Dashboard provisioner/browser, never Sentry SDKs |
| `DATABASE_URL` | PostgreSQL connection | Backend/operator tooling, never client applications |

The opt-in Compose stack binds ingestion to host loopback and does not provision a public domain or TLS terminator. For remote clients, configure an HTTPS reverse proxy or a deliberate tunnel to that endpoint; preserve ingestion paths, query parameters and Sentry authentication headers, and configure request-size/time limits compatible with the intended envelope/minidump sizes. TLS deployment remains operator-owned. Do not embed database credentials or Metabase administrator credentials in a client DSN.

Give each application the DSN through that application's own configuration mechanism. crash-cache does not require a Flutter, Godot or Rust-specific configuration file. Shared-process applications should coordinate SDK initialization and session ownership in their own repository. Use separate projects when distinct test/production datasets are needed: the maintained dashboards filter by project/date and do not currently offer a global environment filter.

`GET /health` proves server reachability, not event persistence. After configuring a client, submit an identified controlled event, confirm it is digested into the intended project, and inspect its platform/release classification. Test sessions, logs, attachments and native crash symbols separately when the application uses them. Archive receipt without successful digestion is not delivery evidence. Consult [validation evidence](../../docs/observability-validation.md) for the tested protocol subset and known limitations.

## Provision the dashboard

Set these variables in a private operator environment, using the running instance and a project already created through `crash-cache project create`:

```text
METABASE_URL
METABASE_ADMIN_EMAIL
METABASE_ADMIN_PASSWORD
METABASE_READONLY_PASSWORD
CRASH_CACHE_PROJECT_ID
```

`METABASE_SOURCE_HOST` defaults to the Compose service `postgres`, and `METABASE_SOURCE_PORT` to `5432`. These are resolved from inside Metabase, not from the machine running the script. Optional `OBSERVABILITY_FROM` and `OBSERVABILITY_UNTIL` are ISO dates, with the latter exclusive; defaults cover the previous 30 days through tomorrow. `OBSERVABILITY_GRAIN` selects the initial chart period (`hour`, `day`, `week` or `month`; default `day`). Use an admin password satisfying the instance's password policy. No credentials are command-line arguments.

```sh
python3 scripts/provision_metabase.py
```

On a fresh instance the script performs initial setup with usage tracking disabled. On an initialized instance it logs in with the supplied credentials. It creates or updates a dedicated read-only PostgreSQL connection, a managed collection, parameterized questions and a selected dashboard with project and date filters plus a chart-period selector. It executes every question before reporting the dashboard URL, then ends its API session. Re-running updates those same objects; an unmanaged name collision or ambiguous duplicate causes a clear failure instead of overwriting unrelated work. Do not add manual cards to this managed dashboard because reprovisioning restores its maintained overview-and-detail layout. Duplicate it first for local customization.

Metabase's database-creation endpoint does not persist `description`; the script marks the newly returned database ID using a separate update. If execution stops between those two requests, inspect the source's identity and connection details. Only after confirming that it is this script's interrupted creation, supply `METABASE_ADOPT_DATABASE_ID` with that exact ID for one recovery run. Adoption checks the expected name, engine, connection and blank/managed description. Remove the override afterward. A matching name alone never establishes ownership.

The overview contains three numeric indicators, activity and session charts with an hour/day/week/month selector, stacked session outcomes, error volume by capture layer and horizontal device bars. Single-day hourly views use compact UTC hour labels with the date retained in the filters; longer ranges show the full UTC date and time. Calendar weeks start Monday. The selector changes only the two time charts; the six original verification tables retain their detailed dimensions in the separate `tables` view. Partial first/last periods include only the selected date range, and missing observation periods are not filled with invented activity. Set `OBSERVABILITY_LABORATORY=1` to label controlled application observations and synthetic loads explicitly; default notes describe generic project data. There is no environment filter: all panels include the selected project/date range, and the activity/device panels require foreground observation events rather than inferring use from errors. The responsive Metabase layout stacks the panels on narrow screens. Chart definitions live in `scripts/dashboard_charts.py` and reuse the maintained SQL semantics. Whole-period active installations and each selected calendar period count installation identities directly rather than adding daily distinct counts. Device bars use short model/OS labels and count identities directly across app versions, OS versions and SDK runtimes; they never sum the version-specific distinct counts in the detail table. One identity observed under different models or OS names may still appear in multiple bars, so the bars must not be summed into a population total. Release-build suffixes are collapsed only for the outcome visualization, where session counts are additive; the detailed release table preserves the original values.

Questions cover session starts, observed active installations, release health, platform/system/device distribution, errors by runtime/component and duplicate cross-layer captures. Empty results are legitimate before observations arrive. See [measurement semantics and required client observations](../../docs/observability.md). SDK runtime (`platform`, such as `dart` or `native`) is different from operating system (`contexts.os.name`, such as Linux or macOS); the dashboard exposes both. Heartbeats and crash recovery must be verified in the application separately.

Run the focused parameter checks and the generated PostgreSQL counting fixture before provisioning changes:

```sh
python3 scripts/test_dashboard_charts.py
python3 scripts/test_dashboard_charts.py --sql > "$CHART_SQL_FIXTURE"
psql "$READONLY_DATABASE_URL" -X -v ON_ERROR_STOP=1 -f "$CHART_SQL_FIXTURE"
```

The SQL runs inside a read-only transaction and shadows tables with synthetic CTEs; it creates no objects and needs no application data. All nine contracts must report `passed = 1`. Cases include one installation crossing days, weeks, months and app versions; project isolation; the exclusive upper bound; missing identity; an error event that must not become activity; and UTC bucketing under a non-UTC database timezone. Session counts remain additive while active-installation counts are recomputed for each period. After provisioning, exercise all four period choices through the real API and browser, verify that only the two time charts change, inspect device label readability, and run the provisioner again to confirm stable card identities. SQL/API success does not establish visual correctness.

The script targets Metabase **0.63.18.2**. It uses the [official API](https://www.metabase.com/docs/latest/api) and [native SQL parameters](https://www.metabase.com/docs/latest/questions/native-editor/sql-parameters). Dashboard creation uses `PUT /api/dashboard/:id` with `dashcards`, not the deprecated cards endpoint. The deployment owner must validate the script against the running pinned image and run it twice to verify idempotence; static checks alone are not deployment evidence.

## Provision the complete authenticated Explorer

The previous provisioner initializes the account and read-only source on a fresh Metabase instance. The current interactive workspace is provisioned separately; there is no manual dashboard editing prerequisite. After backend startup has completed its migrations, install the reporting views with the same Compose project/environment used above:

```sh
podman compose --env-file deploy/observability/.env \
  -f deploy/observability/compose.yml -p crash-cache-observability \
  --profile observability --profile dashboards exec -T postgres \
  psql -U crash_cache -d crash_cache -v ON_ERROR_STOP=1 < docs/sql/investigation.sql
podman compose --env-file deploy/observability/.env \
  -f deploy/observability/compose.yml -p crash-cache-observability \
  --profile observability --profile dashboards exec -T postgres \
  psql -U crash_cache -d crash_cache -v ON_ERROR_STOP=1 \
  -c 'GRANT USAGE ON SCHEMA crash_cache_explorer TO metabase_readonly; GRANT SELECT ON ALL TABLES IN SCHEMA crash_cache_explorer TO metabase_readonly;'
```

Run `python3 scripts/provision_metabase.py` with the private environment described above. In Metabase's Admin → Databases, select **Crash-cache read-only**, synchronize the schema, and note its database ID from the page URL. IDs are instance-specific: do not assume `2`. The API equivalent is `GET /api/database` to find the named source, then `POST /api/database/{id}/sync_schema`; wait until metadata includes all ten `crash_cache_explorer` views. Set `METABASE_DATABASE_ID` to that source ID and `CRASH_CACHE_PROJECT_ID` to the project you created. Then run:

```sh
python3 scripts/provision_explorer.py \
  --database "$METABASE_DATABASE_ID" --project "$CRASH_CACHE_PROJECT_ID" \
  --start 2026-09-01 --end 2026-09-30 \
  --evidence-origin "$METABASE_URL"
```

Choose the desired inclusive UTC date range. `METABASE_URL` must be the absolute browser-facing proxy origin for image previews; use its reachable hostname rather than a container hostname. Open `/dashboard/{home.dashboard_id}` from the printed JSON. No event ID is required: select an occurrence to populate the detail view. Re-run the provisioner to verify stable object IDs. The [Explorer guide](../../docs/explorer.md) documents filtering, authorization and data semantics. Synthetic data is optional and must be inserted explicitly into a dedicated project; fresh deployments do not seed fabricated activity automatically.

## Diagnostic and collection dashboards

Set `OBSERVABILITY_VIEW=diagnostics` and rerun the provisioner to create or update **Diagnostic context and evidence**. Its six cards show error-report counts, breadcrumb coverage and categories, frame/source/variable evidence, native symbolication outcomes and custom-context families. These queries count reports rather than multiplying rows from joined breadcrumbs, frames or contexts. Presence of source or variables is not proof that every frame was captured correctly. The first stored exception stack is the current database scope.

Set `OBSERVABILITY_VIEW=pipeline` to provision **Collection, logs and attachments** after applying the rich-telemetry migration. Its eight cards cover received archives, current pending/failed archives, structured-log severity and daily volume, attachment types and payload bytes, and report-to-attachment correlation. Queue counts are current snapshots restricted to archives received within the selected dates, not historical backlog curves. Log charts use the log timestamp; archive and attachment volume use receipt time. Attachment coverage uses report time and accepts later-arriving metadata for the same project/event.

The default `OBSERVABILITY_VIEW=overview` retains **Application health and installations**. All views share the managed collection and read-only database connection; their cards and public-link identities survive reprovisioning. Provisioning does not enable public sharing. An operator may deliberately share aggregate laboratory dashboards, while admin access remains authenticated. These dashboards do not publish raw log messages, screenshots, source code, variable values, filenames or installation identifiers.

Set `OBSERVABILITY_VIEW=details` to provision **Error investigation**, with capture-layer, app-version and device filters plus a time series, component/type distributions and the latest 200 report summaries. The value `All` clears a dimension filter. Missing metadata is labeled explicitly; filters use each report's own context, not an inferred device from other reports. Set `OBSERVABILITY_VIEW=tables` for **Detailed verification tables**; these technical tables no longer clutter the overview.

For a deliberately shared laboratory, provision all five views, then set `METABASE_ENABLE_PUBLIC_NAVIGATION=1` and run `python3 scripts/dashboard_navigation.py`. This explicitly creates public links for the five managed dashboards and wires navigation. The script defaults to same-origin URLs, so both local and remote access work; optional `METABASE_PUBLIC_BASE_URL` must be an HTTP(S) origin. Run navigation again after reprovisioning views. Ordinary provisioning never enables sharing. The overview's layer/device bars and the data table's app-version/device cells pass the selected project, inclusive/exclusive dates and clicked value to the investigation view. Section links open the section's default filters. Metabase native automatic drill-through is unavailable on public links; maintained custom destinations supply this navigation instead. Navigation preserves public UUIDs, card IDs and layout on repeat runs.

Run `python3 scripts/test_detail_charts.py --sql` for ten filter-isolation contracts and `python3 scripts/test_dashboard_navigation.py` for parameter identity, link templates and origin validation. Browser clicks remain the end-to-end navigation oracle.

Run `python3 scripts/test_diagnostic_charts.py` and execute its `--sql` output with the read-only PostgreSQL connection. Fourteen contracts cover project/time isolation, shared and repeated breadcrumbs, overlapping contexts, empty evidence, late attachment arrival, queue snapshots and UTC grouping under a non-UTC database session. Live query execution and narrow-screen visual inspection remain separate checks.

Attachment metadata is populated when envelopes are processed by the updated backend. Historical raw archives are not retroactively indexed by this migration. A missing metadata row therefore does not prove that the archived payload lacks an attachment. Stored structured logs are separate from breadcrumbs and do not create error issues. Exact envelope retries are deduplicated; differently batched identical log records are preserved because the SDK protocol does not supply a universal log occurrence identifier.

## Versions and lifecycle

Images are pinned to Symbolicator 26.9.0, PostgreSQL 18.6, Python 3.14.7, Metabase 0.63.18.2 and Caddy 2.11.4. Patch tags are explicit but not immutable digests; record resolved image digests with runtime measurements. The builder and runtime images use Rust 1.98.1. The lockfile is resolved against that toolchain; updating it requires testing ingestion, replay and persistence contracts as well as rebuilding the deployment image. The Dockerfile now builds with `--locked` so deployment cannot silently resolve a different dependency set.

The September 27, 2026 version audit selected the [Metabase 0.63.18.2 hotfix image](https://hub.docker.com/r/metabase/metabase/tags?name=v0.63.18.2), including fixes for [row charts with remapped columns](https://github.com/metabase/metabase/pull/82698) and [custom actions in embedded editable dashboards](https://github.com/metabase/metabase/pull/82918). GitHub's latest release entry still names 0.63.18; the exact hotfix image avoids relying on the floating `0.63.18.x` alias. Metabase 0.64 is still beta and is not selected. Follow the [upgrade guide](https://www.metabase.com/docs/latest/installation-and-operation/upgrading-metabase): back up the application database before replacing the image, wait for startup migrations, and verify authenticated queries and evidence routes. Restoring the previous image alone is not a database rollback.

Stop the project with the same Compose file, environment, project name and profiles followed by `down`. Named volumes remain. Do not add `--volumes` unless intentionally deleting the laboratory's database and symbol cache. Keep database exports, raw envelopes, dumps, symbols and operational credentials outside Git. No lifecycle command here operates the separate existing root Compose deployment.
