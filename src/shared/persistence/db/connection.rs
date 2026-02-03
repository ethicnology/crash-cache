use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::time::Duration;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub type DbConnection = PgConnection;

pub type DbPool = Pool<ConnectionManager<DbConnection>>;

pub fn establish_connection_pool(database_url: &str, max_size: u32, timeout_secs: u64) -> DbPool {
    let manager = ConnectionManager::<DbConnection>::new(database_url);
    Pool::builder()
        .max_size(max_size)
        .connection_timeout(Duration::from_secs(timeout_secs))
        .build(manager)
        .expect("Failed to create connection pool")
}

pub fn run_migrations(pool: &DbPool) {
    let mut conn = pool.get().expect("Failed to get connection from pool");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
}
