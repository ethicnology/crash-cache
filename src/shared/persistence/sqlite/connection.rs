use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};

pub type SqlitePool = Pool<ConnectionManager<SqliteConnection>>;

pub fn establish_connection_pool(database_url: &str) -> SqlitePool {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .max_size(10)
        .build(manager)
        .expect("Failed to create connection pool")
}

pub fn run_migrations(pool: &SqlitePool) {
    let mut conn = pool.get().expect("Failed to get connection from pool");

    conn.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS project (
            id TEXT PRIMARY KEY NOT NULL,
            public_key TEXT,
            name TEXT,
            created_at TIMESTAMP NOT NULL
        );

        CREATE TABLE IF NOT EXISTS archive (
            hash TEXT PRIMARY KEY NOT NULL,
            compressed_payload BLOB NOT NULL,
            original_size INTEGER NOT NULL,
            created_at TIMESTAMP NOT NULL
        );

        CREATE TABLE IF NOT EXISTS event (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            project_id TEXT NOT NULL,
            archive_hash TEXT NOT NULL,
            received_at TIMESTAMP NOT NULL,
            processed BOOLEAN NOT NULL DEFAULT FALSE,
            FOREIGN KEY (project_id) REFERENCES project(id),
            FOREIGN KEY (archive_hash) REFERENCES archive(hash)
        );

        CREATE TABLE IF NOT EXISTS processing_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            event_id INTEGER NOT NULL UNIQUE,
            created_at TIMESTAMP NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,
            next_retry_at TIMESTAMP,
            FOREIGN KEY (event_id) REFERENCES event(id)
        );

        CREATE TABLE IF NOT EXISTS report_metadata (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            event_id INTEGER NOT NULL UNIQUE,
            app_version TEXT,
            platform TEXT,
            environment TEXT,
            error_type TEXT,
            error_message TEXT,
            sdk_name TEXT,
            sdk_version TEXT,
            processed_at TIMESTAMP NOT NULL,
            FOREIGN KEY (event_id) REFERENCES event(id)
        );

        CREATE INDEX IF NOT EXISTS idx_event_project_id ON event(project_id);
        CREATE INDEX IF NOT EXISTS idx_event_archive_hash ON event(archive_hash);
        CREATE INDEX IF NOT EXISTS idx_event_processed ON event(processed);
        CREATE INDEX IF NOT EXISTS idx_processing_queue_next_retry ON processing_queue(next_retry_at);
        ",
    )
    .expect("Failed to run migrations");
}
