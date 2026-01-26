use std::io::{self, Write};

use diesel::prelude::*;
use diesel::sql_query;

use crate::shared::persistence::SqlitePool;

const TABLES_TO_CLEAR: &[&str] = &[
    "report",
    "lookup_stacktrace",
    "issue",
    "lookup_exception_message",
    "lookup_device_specs",
    "lookup_platform",
    "lookup_environment",
    "lookup_connection_type",
    "lookup_orientation",
    "lookup_os_name",
    "lookup_os_version",
    "lookup_manufacturer",
    "lookup_brand",
    "lookup_model",
    "lookup_chipset",
    "lookup_locale_code",
    "lookup_timezone",
    "lookup_app_name",
    "lookup_app_version",
    "lookup_app_build",
    "lookup_user",
    "lookup_exception_type",
    "queue",
    "queue_error",
];

pub fn handle(pool: &SqlitePool, yes: bool) {
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

    for table in TABLES_TO_CLEAR {
        let _ = sql_query(format!("DELETE FROM sqlite_sequence WHERE name = '{}'", table))
            .execute(&mut conn);
    }
    println!("   ✓ Sequences reset");

    println!("\n⏳ Re-queuing archives...");

    let result = sql_query(
        "INSERT INTO queue (archive_hash, created_at)
         SELECT hash, datetime('now') FROM archive",
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
