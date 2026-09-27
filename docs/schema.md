# Database Schema

## Entity Relationship Diagram

```mermaid
erDiagram
    %% ============================================
    %% CORE TABLES
    %% ============================================
    
    project {
        INTEGER id PK
        TEXT public_key UK
        TEXT name
        TIMESTAMP created_at
    }
    
    archive {
        TEXT hash PK
        INTEGER project_id FK
        BLOB compressed_payload
        INTEGER original_size "NULL if received compressed"
        TIMESTAMP created_at
    }
    
    queue {
        INTEGER id PK
        TEXT archive_hash FK,UK
        TIMESTAMP created_at
    }
    
    queue_error {
        INTEGER id PK
        TEXT archive_hash FK,UK
        TEXT error
        TIMESTAMP created_at
    }
    
    %% ============================================
    %% SESSION TABLES
    %% ============================================
    
    unwrap_session_status {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_session_release {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_session_environment {
        INTEGER id PK
        TEXT value UK
    }
    
    session {
        INTEGER id PK
        INTEGER project_id FK
        TEXT sid UK
        INTEGER init
        TEXT started_at
        TEXT timestamp
        INTEGER errors
        INTEGER status_id FK
        INTEGER release_id FK
        INTEGER environment_id FK
        TEXT distinct_id "SHA-256 installation identity"
        TEXT sequence "Unsigned logical clock"
        DOUBLE duration "Seconds"
        TEXT abnormal_mechanism
    }
    
    %% ============================================
    %% UNWRAP TABLES
    %% ============================================
    
    unwrap_platform {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_environment {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_os_name {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_os_version {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_manufacturer {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_brand {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_model {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_chipset {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_locale_code {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_timezone {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_connection_type {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_orientation {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_app_name {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_app_version {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_app_build {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_user {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_exception_type {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_device_specs {
        INTEGER id PK
        INTEGER screen_width
        INTEGER screen_height
        REAL screen_density
        INTEGER screen_dpi
        INTEGER processor_count
        INTEGER memory_size
        TEXT archs
    }
    
    unwrap_exception_message {
        INTEGER id PK
        TEXT hash UK
        TEXT value
    }
    
    unwrap_stacktrace {
        INTEGER id PK
        TEXT hash UK
        TEXT fingerprint_hash FK
        JSONB frames
    }
    
    %% ============================================
    %% BREADCRUMB TABLES
    %% ============================================
    
    unwrap_breadcrumb_category {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_breadcrumb_type {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_breadcrumb_level {
        INTEGER id PK
        TEXT value UK
    }
    
    unwrap_breadcrumb {
        INTEGER id PK
        TEXT hash UK
        BIGINT timestamp "epoch ms, nullable"
        INTEGER category_id FK
        INTEGER type_id FK
        INTEGER level_id FK
        JSONB data "merged with breadcrumb.message"
    }
    
    report_breadcrumb {
        INTEGER report_id PK,FK
        INTEGER seq PK
        INTEGER breadcrumb_id FK
    }
    
    %% ============================================
    %% ISSUE TABLE
    %% ============================================
    
    issue {
        INTEGER id PK
        TEXT fingerprint_hash UK
        INTEGER exception_type_id FK
        TEXT title
        TIMESTAMP first_seen
        TIMESTAMP last_seen
        INTEGER event_count
    }
    
    %% ============================================
    %% MAIN REPORT TABLE
    %% ============================================
    
    report {
        INTEGER id PK
        TEXT event_id UK
        TEXT archive_hash FK
        INTEGER timestamp
        TIMESTAMP received_at
        INTEGER project_id FK
        INTEGER platform_id FK
        INTEGER environment_id FK
        INTEGER os_name_id FK
        INTEGER os_version_id FK
        INTEGER manufacturer_id FK
        INTEGER brand_id FK
        INTEGER model_id FK
        INTEGER chipset_id FK
        INTEGER device_specs_id FK
        INTEGER locale_code_id FK
        INTEGER timezone_id FK
        INTEGER connection_type_id FK
        INTEGER orientation_id FK
        INTEGER app_name_id FK
        INTEGER app_version_id FK
        INTEGER app_build_id FK
        INTEGER user_id FK
        INTEGER exception_type_id FK
        INTEGER exception_message_id FK
        INTEGER stacktrace_id FK
        INTEGER issue_id FK
        INTEGER session_id FK
    }
    
    %% ============================================
    %% RELATIONSHIPS
    %% ============================================
    
    archive ||--o{ queue : "queued for"
    archive ||--o{ queue_error : "failed"
    archive ||--o{ report : "stored in"
    
    project ||--o{ archive : "receives"
    project ||--o{ report : "owns"
    
    unwrap_platform ||--o{ report : "platform"
    unwrap_environment ||--o{ report : "environment"
    unwrap_os_name ||--o{ report : "os_name"
    unwrap_os_version ||--o{ report : "os_version"
    unwrap_manufacturer ||--o{ report : "manufacturer"
    unwrap_brand ||--o{ report : "brand"
    unwrap_model ||--o{ report : "model"
    unwrap_chipset ||--o{ report : "chipset"
    unwrap_locale_code ||--o{ report : "locale"
    unwrap_timezone ||--o{ report : "timezone"
    unwrap_connection_type ||--o{ report : "connection"
    unwrap_orientation ||--o{ report : "orientation"
    unwrap_app_name ||--o{ report : "app_name"
    unwrap_app_version ||--o{ report : "app_version"
    unwrap_app_build ||--o{ report : "app_build"
    unwrap_user ||--o{ report : "user"
    unwrap_exception_type ||--o{ report : "exception_type"
    unwrap_exception_type ||--o{ issue : "exception_type"
    
    unwrap_device_specs ||--o{ report : "device_specs"
    unwrap_exception_message ||--o{ report : "exception_msg"
    unwrap_stacktrace ||--o{ report : "stacktrace"
    issue ||--o{ report : "issue"
    
    project ||--o{ session : "tracks"
    session ||--o{ report : "session"
    unwrap_session_status ||--o{ session : "status"
    unwrap_session_release ||--o{ session : "release"
    unwrap_session_environment ||--o{ session : "environment"
    
    report ||--o{ report_breadcrumb : "trail"
    unwrap_breadcrumb ||--o{ report_breadcrumb : "physical bc"
    unwrap_breadcrumb_category ||--o{ unwrap_breadcrumb : "category"
    unwrap_breadcrumb_type ||--o{ unwrap_breadcrumb : "type"
    unwrap_breadcrumb_level ||--o{ unwrap_breadcrumb : "level"
```

## Breadcrumbs

The Sentry SDK attaches an ordered timeline of "breadcrumbs" to every captured event — recent app lifecycle / navigation / HTTP / UI events. Multiple events from the same user session typically share the most recent N breadcrumbs verbatim, so storage uses the same hash-deduped pattern as `unwrap_stacktrace`:

- `unwrap_breadcrumb` — one row per **physical** breadcrumb (timestamp + category + type + level + data). `hash` is SHA256 of a canonical JSON form so the same breadcrumb captured by two events resolves to the same row.
- `report_breadcrumb` — thin join `(report_id, seq, breadcrumb_id)`. `seq` preserves the SDK's chronological ordering (oldest → newest, 0-based). PK is `(report_id, seq)`.
- The breadcrumb's `data` JSONB merges the SDK's `breadcrumb.data` object with `breadcrumb.message` (when present); NULL when both are absent.

## Table Summary

| Category | Tables | Purpose |
|----------|--------|---------|
| **Core** | `project`, `archive`, `queue`, `queue_error` | Project config, raw storage, async processing |
| **Session** | `session`, `unwrap_session_*` | User session tracking and health metrics |
| **Unwrap** | 20 `unwrap_*` tables | Deduplicated string values (normalized) |
| **Breadcrumb** | `unwrap_breadcrumb`, `unwrap_breadcrumb_{category,type,level}`, `report_breadcrumb` | Per-event breadcrumb timeline (hash-deduped physical rows) |
| **Issue** | `issue` | Error grouping by fingerprint |
| **Main** | `report` | Central table with 22 FK references |
| **Analytics** | `bucket_rate_limit_global`, `bucket_rate_limit_dsn`, `bucket_rate_limit_subnet`, `bucket_request_latency` | Aggregated metrics for rate limiting and request performance |

## Data Flow

```mermaid
flowchart LR
    subgraph Ingestion
        A[Sentry SDK] -->|envelope| B[/api/project_id/envelope/]
        B -->|event envelope| C[archive]
        B -->|session-only| S[session]
        C --> D[queue]
    end
    
    subgraph Processing
        D -->|worker| E[DigestReportUseCase]
        E -->|decompress| C
        E -->|extract session| SS[session]
        E -->|parse & normalize| F[unwrap_* tables]
        E -->|extract| G[issue]
        E -->|create| H[report]
        E -->|on error| I[queue_error]
    end
    
    subgraph Storage
        F --> H
        G --> H
        C --> H
        SS -->|link via sid_id| H
        S -->|normalize| J[unwrap_session_* tables]
        SS -->|normalize| J
    end
```

**Note:** Sessions in event envelopes are processed during digest (not ingest), ensuring atomic processing of related data.

## Analytics Tables

### bucket_rate_limit_global
Tracks global request rate over time.

| Column | Type | Description |
|--------|------|-------------|
| id | SERIAL | Primary key |
| bucket_start | TIMESTAMP | Start of time bucket (UNIQUE) |
| hit_count | INTEGER | Number of requests in bucket |

### bucket_rate_limit_dsn
Tracks per-project/DSN request rate.

| Column | Type | Description |
|--------|------|-------------|
| id | SERIAL | Primary key |
| dsn | TEXT | Project DSN key |
| project_id | INTEGER | Project ID (nullable) |
| bucket_start | TIMESTAMP | Start of time bucket |
| hit_count | INTEGER | Number of requests in bucket |

**UNIQUE:** (dsn, bucket_start)

### bucket_rate_limit_subnet
Tracks per-IP/subnet request rate.

| Column | Type | Description |
|--------|------|-------------|
| id | SERIAL | Primary key |
| subnet | TEXT | IP address or subnet |
| bucket_start | TIMESTAMP | Start of time bucket |
| hit_count | INTEGER | Number of requests in bucket |

**UNIQUE:** (subnet, bucket_start)

### bucket_request_latency
Aggregated request latency metrics per endpoint.

| Column | Type | Description |
|--------|------|-------------|
| id | SERIAL | Primary key |
| endpoint | TEXT | API endpoint path |
| bucket_start | TIMESTAMP | Start of time bucket |
| request_count | INTEGER | Number of requests |
| total_ms | INTEGER | Total latency (milliseconds) |
| min_ms | INTEGER | Minimum latency (nullable) |
| max_ms | INTEGER | Maximum latency (nullable) |

**UNIQUE:** (endpoint, bucket_start)

**Note:** These tables are automatically cleaned up based on `ANALYTICS_RETENTION_DAYS` configuration.

## Indexes

| Index | Table | Column(s) | Purpose |
|-------|-------|-----------|---------|
| `idx_archive_project` | archive | project_id | Filter archives by project |
| `idx_report_project` | report | project_id | Filter by project |
| `idx_report_timestamp` | report | timestamp | Time-based queries |
| `idx_report_issue` | report | issue_id | Group by issue |
| `idx_report_user` | report | user_id | Filter by user |
| `idx_unwrap_stacktrace_fingerprint` | unwrap_stacktrace | fingerprint_hash | Find stacktraces by fingerprint |
| `idx_unwrap_stacktrace_frames` | unwrap_stacktrace | frames (GIN) | Query JSON frames content |
| `idx_session_project` | session | project_id | Filter sessions by project |
| `idx_session_status` | session | status_id | Filter sessions by status |
| `idx_session_sid` | session | sid | Find session by sid |
| `idx_report_session` | report | session_id | Find reports by session |
| `idx_unwrap_breadcrumb_timestamp` | unwrap_breadcrumb | timestamp | Time-based breadcrumb queries |
| `idx_report_breadcrumb_breadcrumb` | report_breadcrumb | breadcrumb_id | Reverse lookup: which reports include a given breadcrumb |
| `idx_bucket_rate_limit_global_start` | bucket_rate_limit_global | bucket_start | Time-based cleanup |
| `idx_bucket_rate_limit_dsn_start` | bucket_rate_limit_dsn | bucket_start | Time-based cleanup |
| `idx_bucket_rate_limit_subnet_start` | bucket_rate_limit_subnet | bucket_start | Time-based cleanup |
| `idx_bucket_request_latency_start` | bucket_request_latency | bucket_start | Time-based cleanup |

## Session analytics

See [application sessions and diagnostics](observability.md) for identity, ordering, supported protocol scope and read-only dashboard queries. Session uniqueness is `(project_id, sid)`; event occurrence uniqueness is `(project_id, event_id)`.

## Structured logs and attachments

Migration `20260926000001_rich_telemetry` adds independent log records and an attachment index. These records never manufacture error reports or issues. Existing event/session identities and migrations remain unchanged.

`telemetry_log` stores `project_id`, `archive_hash`, `item_index`, `log_index`, numeric Unix-seconds `timestamp`, nullable `trace_id` and `span_id`, `level`, `body`, nullable `severity_number`, typed `attributes` JSONB and server-side `created_at`. Uniqueness is `(archive_hash, item_index, log_index)`: an exact envelope retry is idempotent, while identical log text in another envelope remains a separate observation. Sentry logs do not supply an occurrence UUID; this deliberately does not promise deduplication of rebatched logs. A trace can contain many distinct logs. Project/time and project/trace indexes support bounded analytics. The accepted envelope format follows [Sentry structured logs](https://develop.sentry.dev/sdk/telemetry/logs/) and Dart SDK 9.30.1: `type: log`, payload `{ "items": [...] }`, with typed scalar or array attributes. Malformed batches are rejected atomically; at most 10,000 logs are admitted per envelope within the existing byte limits.

`attachment_metadata` stores `project_id`, `archive_hash`, normalized nullable `event_id`, `item_index`, nullable `filename`, `attachment_type` and `content_type`, `size_bytes`, and server-side `created_at`. Uniqueness is `(archive_hash, item_index)`. Join a report using both project and event ID; attachment-only envelopes can arrive before their associated event. Join `archive.created_at` for receipt time rather than client event time. Screenshots, scene trees, view hierarchies and opaque Crashpad multipart attachments remain byte-exact inside the compressed archive; only metadata is copied into this index. There is no binary decoding, image display endpoint, or Crashpad breadcrumb-stream interpretation. Repeated attachments in changed envelopes count separately; deduplicate associated reports when computing coverage.

Queryable frame variables and source context remain in `unwrap_stacktrace.frames` JSONB for the first exception. Further exceptions, event extras, and non-normalized fields remain in the raw archive rather than being promoted into dedicated analytics columns. Log-only and mixed session/log envelopes are archived and digested asynchronously; ordinary session-only ingestion retains its existing direct path.
