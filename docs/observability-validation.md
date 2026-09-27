# Observability validation — September 26, 2026

The bounded laboratory test confirmed event and session idempotence, queryable installation counts, device/session correlation and six functioning Metabase questions. These measurements describe this workload and environment. They are not production capacity, mobile SDK overhead, game performance or native symbolication benchmarks.

## Tested identity and environment

| Item | Recorded value |
|---|---|
| Backend source | `1e420da` plus the uncommitted observability working-tree changes under review |
| Benchmark harness SHA-256 | `bedab6300a22baa69acdc20b0be33bab9153a18ea36b04fce09a79edcb84a4e7` |
| Host | Mac mini M4, 24 GiB RAM |
| Container runtime | Podman 6.1.2, Linux VM allocated 6 CPUs and 8 GiB RAM |
| Request path | Host loopback to the containerized laboratory |
| Measured database | PostgreSQL 16.15 (`postgres:16-alpine`) |
| Symbolicator present | 26.9.0; this HTTP ingestion workload did not benchmark it |
| Metabase | 0.63.18 |
| Backend build mode | Release, built with `cargo build --locked --release -j 1` in the pinned Rust 1.93 Docker builder |
| Workload | 100 distinct synthetic events, five direct event retries, three session initializations, six additional mixed/reordered/retried session cases |
| Concurrency | Four concurrent requests for the distinct-event batch |
| Warmup | Three session initialization requests before the event batch |
| Repetitions | One run per rate-limit configuration |
| Request timeout | 10 seconds |

The measured existing laboratory used PostgreSQL 16.15. The separately maintained optional deployment pins PostgreSQL 18.6; its version must not be substituted into this evidence. A source base plus a dirty working tree is less precise than a committed source or binary checksum. The HTTP measurements used local release image `sha256:d806b1da58fb779d3a6802548f5f9b849685f22f5bc36470da2b0b260cb87b32`. The subsequent compressed Crashpad multipart correction is included in source commit `d774d58` and local release image `sha256:2bf42fb4e6e5a2a19ec2c10dcb6ed85be087efb0e5b21268b6338cc6f40bd4e7`; the earlier latency numbers are not a second benchmark of that corrected image. These are laboratory image identities, not published release artifacts.

## HTTP acknowledgement measurements

The first run exercised the existing per-IP limiter. It returned 60 HTTP 200 responses and 54 HTTP 429 responses out of 114 requests. This demonstrates rate-limit behavior; it is not a successful end-to-end ingestion benchmark or a backend saturation point. The run started at 15:08:23 UTC.

The second run, after the laboratory rate-limit override, acknowledged all 114 requests successfully. It started at 15:09:57 UTC. The following figures include the caller, network path and scheduler. Three initialization requests warm the distinct-event batch but remain part of the all-request measurements.

| Measurement | Rate-limited run | Run with laboratory override |
|---|---:|---:|
| Requests | 114 | 114 |
| HTTP 200 | 60 | 114 |
| HTTP 429 | 54 | 0 |
| Failed or invalid acknowledgements | 54 | 0 |
| Complete harness elapsed time | 0.278 s | 0.281 s |
| Distinct-event batch elapsed time | 0.180 s | 0.182 s |
| All-request latency p50 | 6.305 ms | 6.443 ms |
| All-request latency p95 | 12.575 ms | 12.185 ms |
| All-request latency p99 | 18.103 ms | 19.055 ms |
| All-request maximum | 27.668 ms | 22.552 ms |
| Distinct-event latency p50 | 6.386 ms | 6.475 ms |
| Distinct-event latency p95 | 8.703 ms | 8.768 ms |
| Distinct-event latency p99 | 18.071 ms | 18.977 ms |
| Distinct-event maximum | 18.103 ms | 19.055 ms |
| Request bytes sent | 54,692 | 54,595 |
| Uncompressed payload bytes | 70,636 | 70,636 |

The successful 100-event batch produced approximately 548 acknowledgements per second during its 0.182-second burst. This short quotient is not sustainable throughput. There was no steady-state saturation search, long soak, representative production network, repeated statistical trial or concurrent native symbolication load. The rate-limited latency distribution includes rejections; it must not be interpreted as successful-ingestion latency.

An HTTP 200 response proves acknowledgement only. Database verification after queue processing established the following separately; no digest-completion latency was captured.

## Persisted state after the successful run

| Database assertion | Observed |
|---|---:|
| Reports | 100 |
| Distinct event IDs | 100 |
| Issues | 1 |
| Issue occurrence count | 100 |
| Sessions | 3 |
| Distinct installation hashes among those sessions | 3 |
| Pending archives belonging to this run | 0 |
| Queue errors belonging to this run | 0 |

Final session states were exactly `ok` at sequence `0`, `exited` at sequence `2`, and `crashed` at sequence `2`. Older sequence-1 updates and duplicate terminal updates did not replace the final states. An already-known event carrying a newer crashed session preserved that session update without adding an event or issue occurrence. This verifies the expected database result for the controlled retry/reordering workload rather than relying on HTTP status alone.

Focused regression evidence also covered an existing terminal session surviving schema migration and a malformed historical timestamp causing an explicit migration error. The duplicate-event/session regression failed before its fix and passed after the event work moved into a nested transaction, preserving the outer session update. These are bounded correctness proofs, not a claim of full Sentry protocol compatibility.

## Metabase and access verification

The provisioner ran twice against the same instance and retained the same database, dashboard and six question identities. Each question was then executed through the real Metabase API with the selected test project and the half-open UTC date range September 26–27, 2026. All six returned `completed`. Counts include the controlled application tests as well as synthetic inputs; they are not a production population.

| Question | Result rows | Single API round-trip time |
|---|---:|---:|
| Session starts and installations | 3 | 16.72 ms |
| Observed active installations | 3 | 15.67 ms |
| Release health | 3 | 17.97 ms |
| Platforms, systems and devices | 1 | 19.68 ms |
| Errors by runtime and component | 7 | 17.56 ms |
| Duplicate cross-layer captures | 0 | 15.88 ms |

These are individual API timings captured at 15:18:12 UTC, not percentiles or browser rendering measurements. No dashboard interaction or chart rendering latency was measured. An empty duplicate-capture question is expected when no matching repeated logical error identifier exists; event-ID uniqueness and issue counts were checked independently above.

The query connection identified itself as `metabase_readonly`. Privilege inspection confirmed that it was not a superuser, could not create database/schema objects, could select all application tables and had no table-level INSERT, UPDATE, DELETE, TRUNCATE, TRIGGER or REFERENCES privilege. This was verified through read-only catalog queries; no write was attempted.

The device question initially failed to correlate the application's compact 32-hex session tag with the backend's canonical hyphenated UUID. On the same eight observations, the original comparison correlated zero and the corrected comparison correlated all eight. The final question reported one installation, two correlated sessions and zero uncorrelated observations. The comparison validates the accepted UUID forms while preserving both project and installation checks; stored tags are not rewritten.

The [read-only UUID correlation regression](sql/session-correlation-check.sql) returned zero failures across eight cases: compact, hyphenated and uppercase UUIDs matched; different projects, different installations, missing installation identities and malformed UUID inputs did not.

## Reproduce and interpret

Use a disposable project and private environment variables. Keep the DSN, database password, admin credentials, local connection information, raw dumps and raw envelopes outside Git. Start or reuse an isolated stack according to the [laboratory guide](../deploy/observability/README.md), record its actual versions/build mode, then run the maintained harness:

```sh
python3 scripts/benchmark_sentry.py \
  --events 100 --concurrency 4 --retries 5 --timeout 10 \
  --revision "$BACKEND_REVISION" --output "$BENCHMARK_REPORT"
```

`CRASH_CACHE_DSN` is read from the private environment. Use a fresh run identity for an independent trial. Retain the first rate-limit response distribution; if a laboratory override is used for compatibility testing, record it explicitly and do not silently call it a production recommendation. After queue processing, compare reports, issue occurrence count, exact session sequences/statuses and queue state with the generated report's `expected_after_digest`. Do not assume that the harness's initially unverified expected counts are measurements.

Run `python3 scripts/provision_metabase.py` twice with the operator environment described in the laboratory guide. Confirm stable object identities and completion of all six questions. Use the maintained SQL directly when diagnosing results:

```sh
psql "$DATABASE_URL" -v project_id="$PROJECT_ID" \
  -v from="$FROM_TIMESTAMP" -v until="$UNTIL_TIMESTAMP" \
  -f docs/sql/observability.sql
psql "$DATABASE_URL" -f docs/sql/session-correlation-check.sql
```

Use a SELECT-only connection for these commands. The second query must return zero failure rows. The client must emit the documented foreground observations before active-installation counts are meaningful. See [application-session and activity semantics](observability.md) for date-boundary behavior, missing observations and installation-versus-person limitations.

No measurements in this report establish smartphone CPU/memory/battery overhead, a production ingestion capacity, complete Sentry server parity, exact player counts or graphical cross-platform application behavior. The bounded native crash evidence below establishes one resolved function, not complete symbol coverage; broader claims require their own identified artifacts and representative tests.

## Real SDK transport compatibility

A Linux Crashpad client sends a gzip-compressed multipart body, including a roughly 13 MiB native minidump. The initial implementation accepted a plain 3 MiB multipart request but rejected compressed multipart with HTTP 400. Three focused router regressions established this distinction. After bounded decompression before multipart parsing, both supported requests pass and a decompressed payload above the configured limit returns HTTP 413. At that stage, the ordinary Rust suite passed 46 tests, and six explicitly enabled PostgreSQL migration/replay and Symbolicator integration tests passed against isolated services.


Crashpad also supplies SDK scope metadata as MessagePack. A focused regression failed because the backend generated a replacement event identity instead of preserving the one in the MessagePack payload. After decoding that metadata, the same regression passed, together with malformed/ambiguous metadata rejection and the compressed multipart size-limit tests. The subsequent ordinary suite recorded 48 passed and zero failed. Six opt-in integration tests were ignored in that run; their earlier isolated execution is separate evidence, not an additional execution of the updated code.

## Linux ARM64 native crash and classification

Read-only database verification at 16:10 UTC confirmed the controlled Godot 4.7.2 Linux ARM64 run and its subsequent native crash. These were headless functional tests inside the laboratory Linux VM, not a desktop graphics, smartphone or gameplay performance measurement.

| Controlled capture | Stored layer | Stored component | Reports |
|---|---|---|---:|
| Godot handled error | `godot` | `diagnostics` | 1 |
| Rust handled error | `rust` | `diagnostics` | 1 |
| Rust caught panic | `rust` | `diagnostics` | 1 |
| Rust storage failure | `rust` | `storage` | 1 |
| Native null-pointer crash | `native` | `runtime` | 1 |

All four nonfatal reports had one shared installation identity, with no missing identity, and the native crash matched that same installation. Every report recorded Linux with kernel version `7.1.10` and the SDK runtime `native`; the layer/component tags distinguish Godot, Rust and native failures despite their common SDK runtime. These counts establish receipt of each controlled capture once in this selected run, not the absence of duplicate reporting for every possible failure path.

The native report preserved the `native_crash` classification and the `SIGSEGV / SEGV_MAPERR / 0x0` exception. Its stored Symbolicator result was `partial`, with exactly one resolved frame: `sentry::SentryBadCode::crash_with_null_dereference()`. This proves that the actual SDK minidump travelled through ingestion and symbolication to a persisted named function. It does not establish a fully symbolicated stack, source-line coverage, crash-free session accounting, native crash coverage on every supported OS, or symbolication throughput/latency. No new networking, FPS, memory or SDK-overhead measurements accompanied this verification.

## Dashboard visualization follow-up

The same dashboard now includes eight overview questions (three numeric indicators and five charts) followed by the original six verification tables. Hourly foreground observations and session starts use the recorded timestamps. Session outcomes remain separated into exited, crashed, abnormal, unhandled and open states. The device chart shows installations per device/system/app-version grouping, not a sum of unique people. No synthetic history was added to make the charts look populated.

All fourteen questions completed against the real read-only source after provisioning. The existing six question IDs and the dashboard/public-link identity were preserved. A further provisioning run retained all fourteen question IDs and all fifteen dashboard tile IDs, including the laboratory note. Unauthenticated browser rendering was inspected at 1440-pixel desktop and 412-pixel mobile widths using Chrome headless 153.0.8010.12. Panels stacked on mobile without document-level horizontal overflow; detailed tables retain their own scrolling. These checks validate the tested laboratory dashboard, not all browser/platform combinations.


## Rich telemetry and guided investigation follow-up

The September 26 laboratory deployment added typed Sentry log ingestion and attachment metadata while retaining the original compressed envelope or multipart bytes. Replaying the same valid standalone log envelope twice returned HTTP 200 both times and stored one log. Different envelopes are not assumed to contain duplicate log occurrences. Mixed session/log envelopes and opaque Crashpad multipart attachments have focused regression coverage. Existing historical archives were not backfilled; zero indexed attachments can mean that an older archive predates this extraction.

Godot Android exceptions can reference a stack through `exception.thread_id` and `threads.values`. Missing exception stacks are now filled only from an unambiguous matching thread; explicit exception stacks retain precedence. The regression failed with that resolver disconnected and passed after restoration. An actual Pixel 6a script-error receipt subsequently stored local variables from the matching thread stack; source context was absent. This backend result does not establish successful Godot screenshot capture.

The final ordinary Rust test run passed 53 tests. Twelve explicitly enabled tests also passed against isolated PostgreSQL and Symbolicator services, including six rich-telemetry tests, migration rollback/reapply and replay checks. The laboratory image `0142428afaa8847d813e557f296d0b1c12610fd069b307a936c5225755b339d8` was built incrementally from an existing dependency builder and deployed with a successful health response; this is not a clean Docker build claim.

The dashboard layout now separates five views: Overview (8 questions), Diagnostic quality (6), Collection (8), Error investigation (4), and Data tables (6). The original tables moved out of the overview. Navigation retains the existing overview public link, and repeated navigation provisioning preserved all 32 question identities and tile layouts. Sixteen Python contract tests passed, and 33 read-only SQL fixture contracts passed against PostgreSQL, including a non-UTC connection timezone and combined detail filters.

A real browser click on the Pixel 6a device bar opened the investigation dashboard with the selected device, project and date range. Detail filters cover capture layer, application version and device model. The shared investigation contains bounded metadata summaries, not raw error messages, variables or source contents. Custom public URL navigation is intentional because unauthenticated Metabase dashboards do not provide the full authenticated drill-through experience. Section navigation uses each destination's default period; only the configured chart drill-through carries the selected period and project.

Public rendering was inspected at desktop and 412-pixel mobile widths; the tested layouts stacked without document-level horizontal overflow. These are laboratory views over injected test data, not production activity. Active installation counts are not identifiable people, session-owner differences remain documented, and the charts do not establish complete Sentry protocol parity or healthy collection when evidence is absent.


## Generic dashboard classification

The dashboard follow-up removed application-specific allowlists for layer/component names, exception classes, breadcrumb categories and custom context names. Error-layer classification now falls back to standard event platform metadata when a custom layer tag is absent or empty. A read-only PostgreSQL fixture with a Python report and no layer tag failed before this change and passed afterward. Existing detail, diagnostic and collection fixture contracts passed with arbitrary custom classification names preserved. This is a SQL compatibility proof, not a new real Python SDK receipt test.

Generic project notes are the default; laboratory labeling is an explicit deployment option. Optional foreground observations and shared occurrence IDs remain documented client contracts rather than being presented as standard automatic SDK output. Previously recorded public-view metadata-bucketing behavior is superseded: classification names now remain visible across SDKs, while raw diagnostic values remain excluded.

Validation of this follow-up passed 19 Python tests and 34 read-only PostgreSQL query contracts. All 32 managed questions also executed against the laboratory source after reprovisioning; navigation was reapplied to the five existing views with explicit laboratory labeling. No new application build or physical-device test was needed for these dashboard-only changes.

## Authenticated Explorer and realistic demonstration

The authenticated workspace now provides five dashboards with 19 distinct questions: Health overview, Event investigation, Sessions and logs, Releases and devices, and Event details. Fourteen questions use the query builder over nine relational views; five exact-event questions display raw context and evidence. No raw-evidence dashboard or question was publicly shared. Reprovisioning retained the existing collection, question and dashboard identities.

The deterministic original demonstration spans September 6–26, 2026, in a separate explicitly named project. Its manifest SHA-256 is `5830d5cce2909ab9ea7a66d09497c919c3d963b43c43070a1abe0d4c546191ce`. PostgreSQL counts matched the manifest after digest processing: 1,137 reports, including 262 errors and 875 activity observations; 80 supplied identities; 875 sessions; 262 structured logs; and 23 original 16×16 PNG attachments. Session states were 799 exited, 44 crashed, 17 abnormal and 15 open. There were 30 grouped issues across the six synthetic platform profiles. These are demonstration inputs, not real-user population or reliability measurements.

The first unpaced attempt stopped at envelope 1,001 with HTTP 429. The existing limiter passed requests-per-second into tower_governor 0.8's seconds-per-token API: a configured 500 requests/s replenished one token every 500 seconds. A deterministic regression failed with `500s != 2ms` before the reciprocal-period correction and passed afterward. All three limiter scopes now share the conversion and bounded burst arithmetic. The generator also paces requests and implements bounded Retry-After handling. Replaying the same complete 1,997-envelope dataset after deployment retained exactly the manifest counts, including the previously accepted prefix. The old short acknowledgement benchmark above exercised burst capacity; it must not be treated as proof of its former configured sustained rate.

Validation covered 66 Rust library tests and nine PostgreSQL integration tests, including the explicitly enabled Symbolicator cases. The first complete run lacked SYMBOLICATOR_URL; its 64 passing tests were retained and the two missing-environment cases were subsequently rerun successfully with the laboratory service configured. Forty-one Python tests passed. Twelve additional read-only PostgreSQL view contracts checked project-isolated issues, filtered counts, frame/source/variable retention, chronological breadcrumbs, attachment associations and exact project/trace log correlation.

All 20 dashboard-card executions (one question is reused) completed through the authenticated dashboard query API. Individual warmed local round trips were 10.0–40.6 ms; these single samples are not a latency distribution or production capacity estimate. Importantly, direct saved-card query calls did not apply ad hoc MBQL dashboard parameters: verification used the real `/dashboard/{id}/dashcard/{id}/card/{id}/query` route. Selected project/date/environment/version results matched independent PostgreSQL counts: 262 errors for the full demonstration, 47 for production release label 1.1.1 during September 20–26, and zero for a nonexistent project.

A real headless Chromium interaction selected a problem from the overview, carried project and period into its 20-event investigation, and selected an event ID to open that exact occurrence's context, frames, breadcrumbs and correlated log. A separate occurrence exposed its attachment; the authenticated PNG rendered at its original 16×16 dimensions. The download bytes matched the original fixture. HTTP checks returned 401 for anonymous and invalid sessions, 403 for an ordinary analyst, 404 for a missing attachment and 200 for an active administrator. The temporary ordinary-analyst test accounts were deactivated after verification. Binary evidence access is deliberately administrator-wide, not per-project authorization.

The deployed incremental release image is `sha256:c276ec59948e4d4d92fe0ee17364e5eeae89c1710425b9bd12fa632c46cdd5d6`. It reused the earlier cached builder and final base, replacing the Rust executable and migrations; this is not a clean-image reproducibility claim. The optional evidence adapter uses a trusted internal HTTP Metabase authorization URL because the current reqwest feature selection has no TLS backend. Browser HTTPS may terminate independently at the shared reverse proxy.

During the extended authenticated validation, the Metabase container was OOM-killed at its 1.5 GiB limit while configured with a 1 GiB Java heap. Its persistent state remained intact. The laboratory container limit was raised to 2 GiB, with the same heap and automatic restart; the maintained Compose profile now also reserves 2 GiB for a 1 GiB heap. This is workload-specific headroom, not a general capacity guarantee or evidence that one long-running memory sample proves stability.

After the restart, the health endpoint returned `ok`, reprovisioning preserved every Explorer object ID, and the 412-pixel Chromium viewport displayed the 262-error demonstration with no document-level horizontal overflow or JavaScript exception. Detailed tables still require horizontal scrolling on narrow screens; this is an explicit Metabase interaction tradeoff, not a custom mobile inspector. The absolute attachment URL was finally configured for the reachable browser origin; inline rendering had been checked on the laboratory loopback origin before that address-only change.

## Audience and error navigation acceptance — September 26, 2026

The authenticated workspace was reorganized into two entry points with native Metabase tabs: error summaries and progressive event evidence, and a dedicated App usage dashboard. The error list previously included informational activity events with no exception stack; a failing regression test established the missing filter, then passed after restricting the list to grouped error reports. A second Red–Green regression reproduced Metabase's removal of tabs when intermediate dashboard updates omit them. Provisioning now preserves tab IDs and tile membership; a live rerun retained the existing tab IDs and URLs.

The latest stable release check found 0.63.18 already installed; 0.64.0-beta was a prerelease. No version upgrade occurred. Upgrade documentation was reviewed and the application database was backed up during a controlled stop. The laboratory instance uses H2 inside its retained container, not PostgreSQL for Metabase metadata; PostgreSQL is the telemetry source. An existing dashboard ID-generator collision was diagnosed from the H2 primary-key error and advanced past the occupied IDs before creating the new dashboard. A maintained deployment should use a persistent PostgreSQL application database; do not delete the laboratory container assuming its metadata is external.

The original synthetic project was enriched using `--start 2026-09-06 --seed demo-v1 --session-context-only --send`. This admitted and digested 875 new informational context events with exact installation/session links, without creating sessions, errors, logs or attachments. Its enrichment manifest SHA256 is `3b459ce163d5860182c7a60c53e26001070979f355f9d5abb64ea34b26d3d564`. The original v1 envelope bytes remain reproducible; the default new seed is v2 with session context included.

Verified retained counts after enrichment: **2,012 reports = 1,750 activity/context observations + 262 errors**, **80 distinct installation identities**, and **875 logical sessions**. Session outcomes remain 799 exited, 44 crashed, 17 abnormal and 15 open. The audience counters still show 80 installations and 875 starts despite the additional observations, demonstrating that observations are not counted as installations or sessions.

The 15 usage panels were executed through the Metabase query API for day, week and month, plus two empty-project checks: **47 successful query checks**. Session system totals were Linux 303, macOS 145, Windows 144, Android 143 and iOS 140, summing to 875. Collection quality reported zero missing identities, unmatched sessions, conflicting system matches or unknown systems in this deliberately complete synthetic sample. Weekly active-identity buckets were 36, 80, 80 and 80, with the first week truncated by the selected start date; the monthly bucket was 80. These are distinct identities, not sums of daily activity. The three additional detail tables returned system versions, device models and application versions from activity observations.

Validation also passed **58 Python tests** and **11 read-only PostgreSQL audience contracts** under `Pacific/Honolulu`, checking UTC boundaries, project/identity isolation, exact canonical session UUIDs, exclusive upper bounds and weekly uniqueness. The five focused workspace tests were rerun after shortening explanatory text. The new work changes provisioning, queries, demo fixtures and documentation; it does not alter Rust ingestion or require a backend rebuild.

Authenticated browser checks at 1440 px and 412 px verified the usage overview and platform tabs, counters, time series and absence of document-level horizontal overflow or JavaScript exceptions. The session time series uses stacked bars. Selecting an event **message** in investigation opened the matching project/event stack trace, displaying two ordered frames, source location, a source line and captured variables. Direct event detail also displayed its explicit captured-frame count. Detailed tables can still require horizontal scrolling on a phone; native dashboard tabs scroll horizontally when their labels exceed the screen. Evidence display remains dependent on what was received, not proof that every real SDK sends source or variables.

## Workspace UX finalization — September 26, 2026

The workspace now opens on an Overview dashboard with four concise impact counters, collection status, a current/previous comparison, an error trend and a version breakdown. Usage and Errors remain dedicated workflows. Project selectors use a live project-name question; native audience questions use one relative/custom date selector backed by a bounded read-only calendar view. Tables use readable headings, problems are ranked by affected identities then occurrences, technical IDs follow the impact columns, and variables remain in event Context rather than widening the initial stack table. Mobile tab labels and introductory text were shortened.

Primary navigation uses native table-cell click actions to carry the project and raw period token. In Metabase 0.63.18, virtual text/link-card substitution formats date parameters for human reading; it is unsuitable for forwarding `past7days` unchanged. Browser navigation from Overview to Usage and from event detail back to Usage retained the non-default synthetic project and `past7days`. Selecting one problem from the 300-problem sample opened exactly its two occurrences with project and date scope intact. The full provisioning pipeline was repeated and retained all question, dashboard and tab IDs.

Three isolated original synthetic fixtures exercised empty, partial and larger lists. The empty project has no reports or sessions and displays “No activity received for this selection.” The partial project has five reports, two errors, three sessions and two active installation IDs: one activity identity, two session-system matches and one stack are deliberately missing. The larger fixture retains 350 errors grouped into 300 problems across 80 affected identities, with no activity telemetry; its status explicitly distinguishes error-only data from audience activity. These are UI acceptance fixtures, not performance/load benchmarks or real user measurements.

The 18 audience/summary questions completed **54 native query checks** across day/week/month. Non-default project checks distinguish the empty, partial and error-only fixtures. On the main demonstration, September 20–26 compared with September 13–19 produced 80 versus 80 active installation IDs, 283 versus 296 session starts (−13, −4.39%), and 53 versus 145 error occurrences (−92, −63.45%). `past7days` selected the same dates at validation time. A zero previous value produces no invented percentage. Seven summary SQL contracts and five calendar/filter contracts passed under `Pacific/Honolulu`; they cover empty/partial populations, UTC duration, leap-day inclusion, disjoint dates, and invalid scalar suppression. A helper parameter-ID mismatch was reproduced and corrected: standalone native-query requests must use the template-tag UUIDs, while dashboard query routes resolve dashboard parameter IDs through their mappings.

**68 Python tests passed.** The ordinary reporting views were reapplied successfully, including the new calendar; no Rust ingestion code changed and no backend rebuild was needed. Authenticated Chromium checks at 1440 px and 412 px found no document-level horizontal overflow or JavaScript exceptions on the exercised pages. A missing-stack event visibly states “No stack trace received for this event”; the 300-problem table remains ranked and navigable. Wide evidence/comparison tables can still require horizontal scrolling on a phone, and native Metabase table navigation retains its table presentation rather than pretending to be a custom application toolbar.

Limits remain explicit: installation IDs approximate activity, not people or store downloads; the calendar supports 2000–2100 and continuous selected days for audience comparisons; relative previous-period shortcuts can exclude today; collection gaps can change observed counts; differences between periods are not causal evidence. The reporting timezone must be UTC. Project filters select data but are not tenant authorization.


## Screenshot audit and summary priority — September 26, 2026

Actual authenticated Chromium screenshots exposed a weakness that the earlier no-overflow checks missed: at 412 × 915, navigation and introductory/status blocks occupied the entire first screen without displaying a number. The overview now places active identities and errors first, shortens the synthetic-data notice and moves navigation after summary figures. Navigation remains idempotent and preserves native tab membership and project/period links. With the same original project and September 20–26 selection, both 80 active identities and 53 errors are now fully visible in the first viewport (scalar bounds y=509–551 and y=709–751). These are browser viewport checks, not physical-device measurements.

A fresh isolated synthetic project used September 23–26, 20 installations, 24 target sessions per day and seed `ux-screenshot-audit-v2`. The deterministic manifest and independent PostgreSQL counts agreed: 111 reports (83 activity observations and 28 errors), 20 installation identities, 83 logical sessions and 28 logs. Session outcomes were 76 exited, four crashed, two abnormal and one open. The desktop overview displayed 20 active identities, 28 errors, 83 sessions and 17 affected identities. Selecting a problem retained project and date scope; selecting occurrence 3178 opened its SessionExpired context and two captured stack frames. All frames are synthetic fixtures, not a claim about a real application's stack.

The first injection attempt stopped when the Podman VM was found stopped and the browser returned HTTP 502. Its cause was not established. Restarting the retained VM and existing database, backend and Metabase containers restored access without recreating data. Replaying all 193 envelopes with identical IDs yielded the exact manifest counts, with no inflated totals from the interrupted prefix. This recovery is not an uptime guarantee.

The new summary-before-navigation regression failed against the committed pre-correction implementation and passed after the layout correction. All 69 Python tests passed, as did `git diff --check`. Before/after mobile, desktop and stack screenshots were inspected directly and published in a separate laboratory gallery; screenshots and local access credentials remain outside the repository.

The result is useful on desktop and improved on narrow screens, but native Metabase navigation still looks like a table and detailed evidence cards retain considerable whitespace. Wide comparison/evidence tables can require horizontal scrolling. These limitations remain visible; passing SQL and layout tests does not establish a polished custom mobile frontend.


## Physical mobile and fresh-container acceptance — September 27, 2026

A dedicated Pixel 5 running Android 14 and Vanadium 133.0.6943.49.0 exercised the authenticated workspace over an ADB reverse tunnel. Its physical display was 1080 × 2340; the actual browser content viewport was 393 × 722 CSS pixels with device scale factor 2.75. No emulation override was applied. Only this phone was connected, so this is not a three-device validation. Android screenshots, including browser and system bars, were inspected directly. The first screen showed the installation count, with the error count partially below the viewport; one vertical swipe exposed errors and sessions. This corrects the earlier implication that the 412 × 915 emulation established first-screen visibility on all phones.

Touch interaction changed the selected project from the isolated screenshot fixture to the original synthetic project while preserving September 23–26: counters changed to 80 active identities, 32 errors and 159 sessions. Scoped navigation opened Usage, its Platforms tab and the system distribution chart, then Errors and Problems. Selecting a problem opened its five occurrences; selecting a message opened occurrence 1803 with CacheWriteFailure and two captured frames. Narrow stack tables exposed function/location while the source column required horizontal scrolling. Long date-filter labels also extend beyond the visible filter area. The page itself had no horizontal document overflow or captured JavaScript exception during the observed overview loads. These checks establish usable touch navigation, not equivalent comfort to desktop. The phone was verified locked before releasing its reservation.

An independent Compose project started with new named volumes using PostgreSQL 18.6, Metabase 0.63.18, Symbolicator 26.9.0, Python 3.14.7 and Caddy 2.11.4. Two startup defects were reproduced: seven required rate-limit/analytics settings were absent, causing backend restart loops, and the Docker Hub Symbolicator tag returned `manifest unknown`. The settings contract regression failed before correction and passed afterward; Symbolicator now uses its available official GHCR image. A maintained same-origin Caddy proxy replaces the previously external manual proxy prerequisite for dashboard attachment access.

The fresh deployment initialized telemetry migrations and Metabase's separate PostgreSQL application database, created its administrator and source, applied reporting views and provisioned the complete Explorer. Repeated provisioning preserved IDs. All six containers were then removed and recreated with the named volumes retained. Administrator login, seven Explorer dashboards and all 40 question executions succeeded afterward. Six reports, three sessions and one attachment survived; the attachment returned the exact 95-byte PNG for an administrator (200), rejected anonymous/invalid sessions (401), and rejected an authenticated non-administrator (403). The physical Pixel 5 also loaded the recreated instance's overview with two active identities, three errors and three sessions. The deliberately incomplete fixture was flagged by the collection-quality panels.

All 70 Python tests passed after integration. The backend image was reused from the existing tested build: this is fresh-container/volume acceptance, not a clean backend image build. Container recreation with retained storage is distinct from an empty-volume install, which requires provisioning and does not restore old telemetry. No host reboot or backup-restoration test was performed. The temporary audit containers were removed after verification; the existing demonstration was preserved.


## Stable dependency refresh — September 27, 2026

The maintained profiles now pin Metabase 0.63.18.2 and Rust 1.98.1. The root Compose file also pins PostgreSQL 18.6 and Metabase 0.63.18.2 instead of floating tags. Symbolicator 26.9.0, PostgreSQL 18.6, Python 3.14.7 and Caddy 2.11.4 were already current stable versions at verification. Metabase's official Docker image/tag exposed hotfixes not reflected by GitHub's `releases/latest` response; version selection used the exact published tag. The hotfixes cover row charts with remapped columns and embedded editable-dashboard actions, not a new mobile layout.

Compatible Cargo dependencies were refreshed, including Axum 0.8.9, Tokio 1.53.1, Diesel 2.3.13, diesel_migrations 2.3.2, tower-http 0.6.11, UUID 1.26.1 and Clap 4.6.7. The explicit pq-sys constraint moved to 0.7 (resolved 0.7.6), supported by Diesel's dependency range; bundled libpq is 18.6 and OpenSSL is 3.6.3. Reqwest 0.13.5 and rmp-serde 1.3.1 remain current. New incompatible lines of tower-http, sha2 and base64 were not adopted merely to change their version numbers: they alter API/default-behavior contracts and are separate migration decisions. Metabase 64, PostgreSQL 19 and Python 3.15 prereleases were excluded.

A read-only comparison of the updated lockfile against official RustSec advisory ranges found 98 matching-package advisories patched or unaffected, two withdrawn, and no active unpatched match or unresolved range. This used the official advisory data and a local range comparator, not the `cargo audit` CLI; it does not audit operating-system images or establish absence of unknown vulnerabilities. Cargo.lock SHA256: `b1bc9b0db9d8422893da3d9b38d5e5cf375d008bfc13e3a586bbbf9d4de97754`; RustSec revision: `e2111519ba6d14a5da59a7b2e5c8083ae8a37c01`. The old Diesel version was within the affected range of [RUSTSEC-2026-0136](https://rustsec.org/advisories/RUSTSEC-2026-0136.html); updating resolves that dependency finding without asserting crash-cache exposed the affected PostgreSQL COPY options.

All 66 Rust library tests and nine integration tests passed against a separate PostgreSQL 18.6 test database with Symbolicator enabled. The command was `cargo test --locked -- --include-ignored --test-threads=1` with explicit isolated DATABASE_URL and SYMBOLICATOR_URL. Serialization matters because existing database tests truncate their fixture tables. An initial invocation without the database environment failed its connection preconditions; the configured full run passed without skips. All 70 Python tests passed. Clippy was unavailable in the installed host toolchain and was not installed implicitly.

Before upgrading the isolated Metabase instance, both its metadata and telemetry databases were exported using `pg_dump -Fc`. The PostgreSQL-backed upgrade preserved administrator access, seven Explorer dashboards, all 40 executable questions, six reports, three sessions and one attachment. The runtime reported v0.63.18.2 / 7387f79; the pulled image manifest was `sha256:21a98ee8705f22d0141b1719f1062670b932f3375ffc49f7018de6920a19a0df`. The authenticated PNG matched all 95 fixture bytes; anonymous and invalid sessions returned 401. A Chromium check on the upgraded instance rendered the overview without a JavaScript exception or document overflow and followed the scoped Errors link. This hotfix browser check used a 412 × 915 viewport; the separate physical-device results above remain distinct.

The full maintained Dockerfile then built the Linux ARM64 backend with Rust 1.98.1 and `cargo build --locked --release -j 1`; the release compilation took 4m06s. The resulting image was `sha256:604d54cc88a1041db60ba718a950ae818bf7fcf6afe7cb4715da954b4bb1d379`. This build copied the current manifests and source rather than replacing a binary in an old runtime image. Replacing the isolated backend with that image preserved health, all 40 dashboard queries and authenticated attachment bytes. Two identical new controlled envelopes returned 200 and produced exactly one report. The root Compose file passed configuration parsing; runtime acceptance used the opt-in observability profile. The existing demonstration deployment was not replaced by these isolated upgrade tests.
