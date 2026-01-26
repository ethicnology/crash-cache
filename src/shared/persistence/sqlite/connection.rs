use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub type SqlitePool = Pool<ConnectionManager<SqliteConnection>>;

pub fn establish_connection_pool(database_url: &str) -> SqlitePool {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder()
        .max_size(10)
        .build(manager)
        .expect("Failed to create connection pool");

    // Enable WAL mode for better concurrent read/write performance
    {
        let mut conn = pool.get().expect("Failed to get connection for WAL setup");
        diesel::sql_query("PRAGMA journal_mode=WAL")
            .execute(&mut conn)
            .expect("Failed to enable WAL mode");
    }

    pool
}

pub fn run_migrations(pool: &SqlitePool) {
    let mut conn = pool.get().expect("Failed to get connection from pool");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
}
