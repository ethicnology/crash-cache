use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

#[cfg(feature = "sqlite")]
use diesel::{RunQueryDsl, sqlite::SqliteConnection};

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;

#[cfg(feature = "sqlite")]
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/sqlite");

#[cfg(feature = "postgres")]
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/postgres");

#[cfg(feature = "sqlite")]
pub type DbConnection = SqliteConnection;

#[cfg(feature = "postgres")]
pub type DbConnection = PgConnection;

pub type DbPool = Pool<ConnectionManager<DbConnection>>;

// Backward compatibility alias
pub type SqlitePool = DbPool;

pub fn establish_connection_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<DbConnection>::new(database_url);
    let pool = Pool::builder()
        .max_size(10)
        .build(manager)
        .expect("Failed to create connection pool");

    #[cfg(feature = "sqlite")]
    {
        // Enable WAL mode for better concurrent read/write performance
        let mut conn = pool.get().expect("Failed to get connection for WAL setup");
        diesel::sql_query("PRAGMA journal_mode=WAL")
            .execute(&mut conn)
            .expect("Failed to enable WAL mode");
    }

    pool
}

pub fn run_migrations(pool: &DbPool) {
    let mut conn = pool.get().expect("Failed to get connection from pool");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
}
