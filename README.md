# crash-cache

A lightweight, self-hosted Sentry-compatible error tracking backend. Drop-in replacement focused on high-throughput ingestion and efficient storage.

## Why

Sentry's hosted service is expensive at scale. Self-hosting official Sentry requires significant infrastructure. crash-cache is a single binary with PostgreSQL that handles Sentry SDK payloads with minimal operational overhead.

## Features

- **Sentry SDK compatible** - Works with existing Sentry client SDKs
- **High throughput** - Async ingest with deferred processing; hash-based deduplication
- **PostgreSQL optimized** - Native PostgreSQL support with RETURNING clauses and transactions
- **Issue grouping** - Automatic fingerprinting based on in-app stack frames
- **Session tracking** - Full Sentry Session support (crashes, errors, release health)
- **Rate limiting** - Configurable global, per-IP, and per-project limits with burst capacity
- **Proper HTTP semantics** - Correct status codes (503 for DB issues, 422 for compression, etc.)
- **Fully configurable** - All limits and timeouts configurable via environment variables
- **Production-ready** - No panics, comprehensive error handling, transactional processing
- **Bundled dependencies** - No system libpq or openssl required (bundled via Cargo.toml)
- **Docker ready** - Full docker-compose setup with PostgreSQL and Metabase analytics

## Architecture

```
Ingest (fast path)              Digest (background)
─────────────────               ───────────────────
HTTP POST                       Worker (every N sec)
    │                               │
    ▼                               ▼
Validate DSN key                Dequeue batch
    │                               │
    ▼                               ▼
Compress if needed              Decompress + parse
    │                               │
    ▼                               ▼
Hash compressed bytes           Extract metadata
    │                               │
    ▼                               ▼
Store archive + enqueue         Normalize to dimensions
                                    │
                                    ▼
                                Create report + issue
```

Ingest is optimized for speed: validate, compress, hash, store, respond. Heavy parsing and normalization happens asynchronously.

## Quick Start

### Option 1: Docker Compose (Recommended)

**Prerequisites:**
- Docker and Docker Compose
- 2GB+ RAM available

**Steps:**

1. Clone and configure:
```bash
git clone https://github.com/ethicnology/crash-cache.git
cd crash-cache

# Copy and edit environment file
cp .env.example .env
# Edit .env: Change POSTGRES_PASSWORD, METABASE_PASSWORD, and ports if needed
```

2. Start all services:
```bash
docker-compose up -d
```

This starts three services:
- **PostgreSQL** - Database with separate users for crash-cache and Metabase (internal only, not exposed to host)
- **crash-cache-server** (port 3000) - API server (configurable via `CRASH_CACHE_PORT`)
- **Metabase** (port 3001) - Analytics dashboard (configurable via `METABASE_PORT`)

3. Create a project:
```bash
docker exec -it crash-cache-server crash-cache project create --name "my-app"
# Output: DSN: http://<key>@localhost:3000/<project_id>
```

4. Configure your Sentry SDK with the DSN from step 3.

5. Access Metabase at http://localhost:3001 for analytics dashboards
   - Connect to PostgreSQL: host=postgres, database=crash_cache, user=metabase_readonly

### Option 2: Native Build

**Prerequisites:**
- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- PostgreSQL server running
- **No system libpq or openssl required** - dependencies are bundled

**Steps:**

1. Clone and build:
```bash
git clone https://github.com/ethicnology/crash-cache.git
cd crash-cache
cargo build --release
```

2. Configure:
```bash
cp .env.example .env
# Edit .env: Set DATABASE_URL to your PostgreSQL connection string
```

3. Create a project:
```bash
./target/release/crash-cache project create --name "my-app"
# Output: DSN: http://<key>@localhost:3000/<project_id>
```

4. Run server:
```bash
./target/release/crash-cache serve
```

5. Configure your Sentry SDK with the DSN from step 3.

## Configuration

All configuration is done via environment variables (or `.env` file). See `.env.example` for full documentation with sizing profiles (SMALL / MEDIUM / LARGE).

### Docker Compose Variables

These are only used by `docker-compose.yml`.

| Variable | Default | Description |
|----------|---------|-------------|
| `POSTGRES_PASSWORD` | `changeme` | PostgreSQL superuser password |
| `METABASE_PASSWORD` | `changeme_metabase` | Password for Metabase DB users |
| `METABASE_PORT` | `3001` | Host port for the Metabase dashboard |

### Application Variables

These are read by the crash-cache binary. Docker Compose forwards them automatically.

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | — | PostgreSQL connection string (auto-set by Docker Compose) |
| `CRASH_CACHE_HOST` | `0.0.0.0` | Listen address |
| `CRASH_CACHE_PORT` | `3000` | Listen port (also used as Docker host port) |
| `DATABASE_POOL_SIZE` | `30` | Max concurrent database connections |
| `DATABASE_POOL_TIMEOUT_SECS` | `20` | Connection acquire timeout (returns 503 if exceeded) |
| `MAX_COMPRESSED_PAYLOAD_BYTES` | `50 * 1024` | Max gzip payload size (supports math expressions) |
| `MAX_UNCOMPRESSED_PAYLOAD_BYTES` | `200 * 1024` | Max raw JSON size after decompression |
| `WORKER_INTERVAL_SECS` | `60` | Background worker cycle interval (seconds) |
| `WORKER_REPORTS_BATCH_SIZE` | `100` | Archives to process per worker cycle |
| `MAX_CONCURRENT_COMPRESSIONS` | `12` | Max parallel gzip operations (2-3× CPU cores) |
| `RATE_LIMIT_REQUESTS_PER_SEC` | `800` | Global rate limit (0 = disabled) |
| `RATE_LIMIT_PER_IP_PER_SEC` | `30` | Per-IP rate limit (0 = disabled) |
| `RATE_LIMIT_PER_PROJECT_PER_SEC` | `500` | Per-project rate limit (0 = disabled) |
| `RATE_LIMIT_BURST_MULTIPLIER` | `2` | Burst multiplier (2 = allow 2× limit briefly) |
| `ANALYTICS_FLUSH_INTERVAL_SECS` | `10` | Metrics flush interval (seconds) |
| `ANALYTICS_RETENTION_DAYS` | `30` | Auto-delete analytics older than N days |
| `ANALYTICS_BUFFER_SIZE` | `20000` | Internal metrics channel buffer |

## Docker Compose Architecture

The `docker-compose.yml` provides a complete deployment:

```yaml
Services:
  postgres:
    - Main database (crash_cache DB)
    - Metabase storage (metabase DB)
    - Three users:
      * crash_cache (app user)
      * metabase_app (Metabase admin)
      * metabase_readonly (analytics queries)
    - Internal only (not exposed to host)
  
  crash-cache-server:
    - API server on CRASH_CACHE_PORT (default 3000)
    - Health checks every 30s
    - Auto-restarts on failure
  
  metabase:
    - Analytics UI on METABASE_PORT (default 3001)
    - Connect to crash_cache DB with metabase_readonly user
```

**Security Features:**
- Separate passwords for postgres and Metabase (set in `.env`)
- Read-only database user for analytics queries
- No exposed credentials (all via environment variables)

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `POST /api/{project_id}/store/` | Sentry store endpoint (JSON) |
| `POST /api/{project_id}/envelope/` | Sentry envelope endpoint |
| `GET /health` | Health check with cached stats |

## CLI Commands

```bash
# Server
crash-cache serve                      # Start the API server

# Project management
crash-cache project create [--name NAME] [--key KEY]
crash-cache project list
crash-cache project delete <id>

# Archive management
crash-cache archive export [-o FILE]   # Export to JSONL
crash-cache archive import [-i FILE]   # Import from JSONL
crash-cache ruminate                   # Re-digest all archives
```

## Database

PostgreSQL with normalized schema:

- `archive` - Compressed payloads (content-addressed by hash)
- `queue` / `queue_error` - Async processing queue
- `report` - Normalized crash data (~25 dimension FKs)
- `issue` - Error grouping by stack fingerprint
- `unwrap_*` - Dimension tables (platform, os, device, app, etc.) - 20+ tables
- `session` - Release health tracking

**Migrations run automatically on startup.** See [docs/schema.md](docs/schema.md) for full details.

## Performance & Reliability

### Performance Optimizations
- **Fast ingest path** - Avoids decompression when client sends gzip
- **Content-addressed storage** - Deduplicates identical payloads by hash
- **Dimension tables** - Minimizes storage for repetitive strings (OS, device, platform, etc.)
- **Batch processing** - Worker processes events in configurable batches
- **PostgreSQL RETURNING** - Eliminates follow-up SELECT queries after INSERT
- **Transaction support** - Reduces 24+ transactions per event to 1
- **Semaphore limiting** - Controls CPU-bound compression concurrency
- **Single connection per request** - HTTP handlers use one connection throughout request lifecycle
- **Background health refresh** - Health endpoint returns cached stats (no DB queries in request path)
- **Project validation cache** - 60-second TTL cache reduces repeated project key lookups
- **Calculated metrics** - Orphaned archives computed via arithmetic instead of expensive queries
- **Bundled dependencies** - pq-sys and openssl-sys bundled to avoid system dependency issues

### Reliability Features
- **No panics** - All repository `.expect()` calls eliminated
- **Transactional processing** - Digest operations are atomic (all-or-nothing)
- **Connection pooling** - Configurable pool size with timeout protection
- **Proper error codes** - Database issues return 503, compression errors return 422
- **Graceful degradation** - Health check returns 503 when DB unavailable
- **Error propagation** - Session failures properly return HTTP errors

### Build Optimizations
- **Memory-efficient Docker build** - Reduced opt-level for dependencies avoids OOM
- **Single-job builds** - `CARGO_BUILD_JOBS=1` prevents memory exhaustion
- **Bundled native libraries** - No system libpq or openssl dependencies required
- **Multi-stage Docker** - Minimal runtime image with only necessary components

## SDK Integration

crash-cache accepts standard Sentry SDK payloads. Configure any Sentry SDK with your crash-cache DSN:

| Platform | Documentation |
|----------|---------------|
| JavaScript | https://docs.sentry.io/platforms/javascript/ |
| Python | https://docs.sentry.io/platforms/python/ |
| Rust | https://docs.sentry.io/platforms/rust/ |
| Go | https://docs.sentry.io/platforms/go/ |
| Java | https://docs.sentry.io/platforms/java/ |
| .NET | https://docs.sentry.io/platforms/dotnet/ |
| PHP | https://docs.sentry.io/platforms/php/ |
| Ruby | https://docs.sentry.io/platforms/ruby/ |
| Flutter | https://docs.sentry.io/platforms/flutter/ |
| React Native | https://docs.sentry.io/platforms/react-native/ |
| iOS | https://docs.sentry.io/platforms/apple/ |
| Android | https://docs.sentry.io/platforms/android/ |

Replace the DSN in the SDK configuration with the one from `crash-cache project create`.

## Analytics with Metabase

When using Docker Compose, Metabase is automatically deployed for analytics dashboards:

1. Access Metabase at http://localhost:3001
2. Complete initial setup (create admin account)
3. Add PostgreSQL database connection:
   - **Database type:** PostgreSQL
   - **Host:** postgres
   - **Port:** 5432
   - **Database name:** crash_cache
   - **Username:** metabase_readonly
   - **Password:** (your METABASE_PASSWORD from .env)

The `metabase_readonly` user has SELECT-only access to the crash_cache database for security.

**Example dashboards you can build:**
- Error rate by project/platform/version over time
- Top issues by event count
- User impact analysis (unique users affected)
- Session health metrics (crash-free sessions rate)
- Geographic distribution of errors

## Recent Improvements

### Deployment & Build (v0.4.0)
- **Bundled dependencies** - pq-sys and openssl-sys now bundled (no system libpq/openssl needed)
- **Docker Compose setup** - Complete three-service deployment (postgres, crash-cache, metabase)
- **Single .env file** - Unified configuration for all services
- **Optimized Docker build** - Memory-efficient compilation (reduced opt-level, single job)
- **Metabase integration** - Built-in analytics dashboard support with read-only user
- **Security improvements** - Separate passwords for postgres and metabase
- **Configuration consolidation** - Removed old variable names, standardized naming

### Concurrency Optimizations (v0.3.0)
- **Background health stats** - Health endpoint now returns cached stats refreshed every 60s
  - Eliminated expensive DB queries from request path (100-500ms → <1ms)
  - No more TOCTOU races or thundering herd on health checks
- **Calculated orphaned metric** - Simple arithmetic instead of full table scan with 3 NOT EXISTS
- **Project validation cache** - 60s TTL cache reduces validation queries by 60x at high RPS
- **Single connection per request** - Reduced from 4 connections to 1 per HTTP request
  - Handlers get connection once and pass through entire request lifecycle
  - Reduced connection pool contention significantly

### PostgreSQL Migration (v0.2.0)
- **Migrated from SQLite** - PostgreSQL-only for true concurrent writes without database sharding
  - SQLite's single-writer bottleneck would require sharding that breaks JOINs
  - PostgreSQL provides native write concurrency while preserving relational integrity
- **Transaction support** - Atomic digest processing with rollback on failure
- **RETURNING clauses** - Optimized INSERT queries eliminate follow-up SELECTs
- **Connection pooling** - Configurable pool with timeout protection

### Error Handling Overhaul (v0.2.0)
- **HTTP status codes** - Proper semantics (503, 422, 409, 400, etc.)
- **No more panics** - All `.expect()` calls removed from critical paths
- **Error propagation** - Session failures now return HTTP errors
- **Health check** - Returns 503 when database is unavailable

### Configuration Management (v0.2.0)
- **Comprehensive configuration** - Database pool, payload limits, batch sizes, rate limiting, analytics
- **Environment variable support** - All configuration via .env file
- **Backward compatibility** - Automatic fallback to old variable names with deprecation warnings
- **Expression parsing** - Supports multiplication expressions (e.g., `50 * 1024` for readability)

## License

AGPL-3.0 - See [LICENSE](LICENSE) for details.
