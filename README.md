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

### Prerequisites

**PostgreSQL:**
```bash
# macOS (installs libpq and optionally postgresql server)
brew install libpq
# Add to PATH: export PATH="/opt/homebrew/opt/libpq/bin:$PATH"

# Ubuntu/Debian (libpq-dev for building, postgresql for running locally)
sudo apt-get install libpq-dev
# Optional: sudo apt-get install postgresql  (if running DB locally)

# Fedora/RHEL
sudo dnf install libpq-devel
# Optional: sudo dnf install postgresql  (if running DB locally)
```

### Build and Run

```bash
# Build
cargo build --release

# Create a project
crash-cache-cli project create --name "my-app"
# Output: DSN: http://<key>@localhost:3000/<project_id>

# Run server
crash-cache

# Configure your Sentry SDK with the DSN
```

## Configuration

Environment variables (or `.env` file):

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgresql://...` | PostgreSQL connection string |
| `SERVER_HOST` | `0.0.0.0` | Listen host |
| `SERVER_PORT` | `3000` | Listen port |
| `WORKER_INTERVAL_SECS` | `60` | Digest worker interval |
| `WORKER_BUDGET_SECS` | `50` | Max processing time per tick |
| `MAX_CONCURRENT_COMPRESSIONS` | `16` | Concurrent compression limit |
| `RATE_LIMIT_GLOBAL_PER_SEC` | `100` | Global requests/sec (0=off) |
| `RATE_LIMIT_PER_IP_PER_SEC` | `10` | Per-IP requests/sec (0=off) |
| `RATE_LIMIT_PER_PROJECT_PER_SEC` | `50` | Per-project requests/sec (0=off) |

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `POST /api/{project_id}/store/` | Sentry store endpoint (JSON) |
| `POST /api/{project_id}/envelope/` | Sentry envelope endpoint |
| `GET /health` | Health check with stats |

## CLI

```bash
crash-cache-cli project create [--name NAME] [--key KEY]
crash-cache-cli project list
crash-cache-cli project delete <id>
crash-cache-cli ruminate [--yes]  # Reprocess all archives
```

## Database

PostgreSQL with normalized schema:

- `archive` - Compressed payloads
- `queue` / `queue_error` - Processing queue
- `report` - Normalized crash data (~25 dimension FKs)
- `issue` - Grouped by stack fingerprint
- `unwrap_*` - Dimension tables (platform, os, device, app, etc.)
- `session` - Release health tracking

Migrations run automatically on startup.

## Performance & Reliability

### Performance Optimizations
- **Fast ingest path** - Avoids decompression when client sends gzip
- **Content-addressed storage** - Deduplicates identical payloads by hash
- **Dimension tables** - Minimizes storage for repetitive strings (OS, device, platform, etc.)
- **Batch processing** - Worker processes events in configurable batches
- **PostgreSQL RETURNING** - Eliminates follow-up SELECT queries after INSERT
- **Transaction support** - Reduces 24+ transactions per event to 1
- **Semaphore limiting** - Controls CPU-bound compression concurrency

### Reliability Features
- **No panics** - All repository `.expect()` calls eliminated
- **Transactional processing** - Digest operations are atomic (all-or-nothing)
- **Connection pooling** - Configurable pool size with timeout protection
- **Proper error codes** - Database issues return 503, compression errors return 422
- **Graceful degradation** - Health check returns 503 when DB unavailable
- **Error propagation** - Session failures properly return HTTP errors

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

Replace the DSN in the SDK configuration with the one from `crash-cache-cli project create`.

## Recent Improvements

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
- **7 new parameters** - Database pool, payload limits, batch sizes, rate limiting
- **No defaults** - All configuration explicit in `.env` file
- **Fully tunable** - Optimize for your workload and infrastructure

See [CHANGELOG.md](CHANGELOG.md) for complete details.

## License

AGPL-3.0 - See [LICENSE](LICENSE) for details.
