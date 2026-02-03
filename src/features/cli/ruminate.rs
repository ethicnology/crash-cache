use std::io::{self, Write};

use diesel::prelude::*;
use diesel::sql_query;

use crate::shared::persistence::DbPool;

const TABLES_TO_CLEAR: &[&str] = &[
    "report",
    "unwrap_stacktrace",
    "issue",
    "unwrap_exception_message",
    "unwrap_device_specs",
    "unwrap_platform",
    "unwrap_environment",
    "unwrap_connection_type",
    "unwrap_orientation",
    "unwrap_os_name",
    "unwrap_os_version",
    "unwrap_manufacturer",
    "unwrap_brand",
    "unwrap_model",
    "unwrap_chipset",
    "unwrap_locale_code",
    "unwrap_timezone",
    "unwrap_app_name",
    "unwrap_app_version",
    "unwrap_app_build",
    "unwrap_user",
    "unwrap_exception_type",
    "queue",
    "queue_error",
    // Session tables
    "session",
    "unwrap_session_status",
    "unwrap_session_release",
    "unwrap_session_environment",
    // Analytics bucket tables
    "bucket_rate_limit_global",
    "bucket_rate_limit_dsn",
    "bucket_rate_limit_subnet",
    "bucket_request_latency",
];

pub fn handle(pool: &DbPool, yes: bool) {
    let mut conn = pool.get().expect("Failed to get connection");

    let archive_count: i64 = sql_query("SELECT COUNT(*) as count FROM archive")
        .get_result::<CountResult>(&mut conn)
        .map(|r| r.count)
        .unwrap_or(0);

    println!("\n🐄 RUMINATE - Re-digest all archives from scratch\n");
    println!("This will:");
    println!("  ✗ DELETE all data from {} tables:", TABLES_TO_CLEAR.len());
    for table in TABLES_TO_CLEAR {
        println!("      - {}", table);
    }
    println!("\n  ✓ KEEP intact:");
    println!("      - archive ({} entries)", archive_count);
    println!("      - project");
    println!("\n  → RE-QUEUE {} archives for processing\n", archive_count);

    if !yes {
        print!("Are you sure? [y/N] ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return;
        }
    }

    println!("\n⏳ Clearing tables...");

    for table in TABLES_TO_CLEAR {
        match sql_query(format!("DELETE FROM {}", table)).execute(&mut conn) {
            Ok(rows) => println!("   {} - {} rows deleted", table, rows),
            Err(e) => eprintln!("   {} - ERROR: {}", table, e),
        }
    }

    println!("\n⏳ Resetting auto-increment counters...");

    #[cfg(feature = "sqlite")]
    {
        for table in TABLES_TO_CLEAR {
            let _ = sql_query(format!(
                "DELETE FROM sqlite_sequence WHERE name = '{}'",
                table
            ))
            .execute(&mut conn);
        }
    }

    #[cfg(feature = "postgres")]
    {
        for table in TABLES_TO_CLEAR {
            let _ = sql_query(format!(
                "ALTER SEQUENCE IF EXISTS {}_id_seq RESTART WITH 1",
                table
            ))
            .execute(&mut conn);
        }
    }

    println!("   ✓ Sequences reset");

    println!("\n⏳ Re-queuing archives...");

    #[cfg(feature = "sqlite")]
    let result = sql_query(
        "INSERT INTO queue (archive_hash, created_at)
         SELECT hash, datetime('now') FROM archive",
    )
    .execute(&mut conn);

    #[cfg(feature = "postgres")]
    let result = sql_query(
        "INSERT INTO queue (archive_hash, created_at)
         SELECT hash, NOW() FROM archive",
    )
    .execute(&mut conn);

    match result {
        Ok(count) => {
            println!("   ✓ {} archives queued for processing", count);
            println!("\n🎉 Done! The DigestWorker will process them automatically.");
        }
        Err(e) => {
            eprintln!("   ✗ Failed to queue archives: {}", e);
        }
    }
}

#[derive(QueryableByName)]
struct CountResult {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}
