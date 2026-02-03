use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use clap::Subcommand;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

use crate::shared::persistence::DbPool;
use crate::shared::persistence::db::models::ArchiveModel;
use crate::shared::persistence::db::schema::archive;

#[derive(Subcommand)]
pub enum ArchiveCommand {
    /// Export archives to JSONL (base64-encoded blobs)
    Export {
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import archives from JSONL
    Import {
        /// Input file (default: stdin)
        #[arg(short, long)]
        input: Option<String>,
        /// Skip existing archives (no error on duplicate)
        #[arg(long, default_value = "true")]
        skip_existing: bool,
    },
    /// View a decompressed archive by hash
    View {
        /// Archive hash
        hash: String,
    },
}

#[derive(Serialize, Deserialize)]
struct ArchiveRecord {
    hash: String,
    project_id: i32,
    compressed_payload: String, // base64
    original_size: Option<i32>,
    created_at: String,
}

pub fn handle(command: ArchiveCommand, pool: &DbPool) {
    match command {
        ArchiveCommand::Export { output } => export(pool, output),
        ArchiveCommand::Import {
            input,
            skip_existing,
        } => import(pool, input, skip_existing),
        ArchiveCommand::View { hash } => view(pool, hash),
    }
}

fn export(pool: &DbPool, output: Option<String>) {
    let mut conn = pool.get().expect("Failed to get connection");

    let archives: Vec<ArchiveModel> = archive::table
        .select(ArchiveModel::as_select())
        .load(&mut conn)
        .expect("Failed to load archives");

    let writer: Box<dyn Write> = match output {
        Some(path) => {
            let file = File::create(&path).expect("Failed to create output file");
            eprintln!("Exporting to: {}", path);
            Box::new(BufWriter::new(file))
        }
        None => Box::new(io::stdout()),
    };
    let mut writer = writer;

    let mut count = 0;
    for arch in archives {
        let record = ArchiveRecord {
            hash: arch.hash,
            project_id: arch.project_id,
            compressed_payload: BASE64.encode(&arch.compressed_payload),
            original_size: arch.original_size,
            created_at: arch.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
        };

        let line = serde_json::to_string(&record).expect("Failed to serialize");
        writeln!(writer, "{}", line).expect("Failed to write");
        count += 1;
    }

    eprintln!("Exported {} archives", count);
}

fn import(pool: &DbPool, input: Option<String>, skip_existing: bool) {
    let mut conn = pool.get().expect("Failed to get connection");

    let reader: Box<dyn BufRead> = match input {
        Some(path) => {
            let file = File::open(&path).expect("Failed to open input file");
            eprintln!("Importing from: {}", path);
            Box::new(BufReader::new(file))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for (line_num, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Line {}: read error: {}", line_num + 1, e);
                errors += 1;
                continue;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let record: ArchiveRecord = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Line {}: parse error: {}", line_num + 1, e);
                errors += 1;
                continue;
            }
        };

        let payload = match BASE64.decode(&record.compressed_payload) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Line {}: base64 decode error: {}", line_num + 1, e);
                errors += 1;
                continue;
            }
        };

        let created_at =
            chrono::NaiveDateTime::parse_from_str(&record.created_at, "%Y-%m-%dT%H:%M:%S")
                .unwrap_or_else(|_| chrono::Utc::now().naive_utc());

        let model = ArchiveModel {
            hash: record.hash,
            project_id: record.project_id,
            compressed_payload: payload,
            original_size: record.original_size,
            created_at,
        };

        let result = if skip_existing {
            #[cfg(feature = "sqlite")]
            let res = diesel::insert_or_ignore_into(archive::table)
                .values(&model)
                .execute(&mut conn);

            #[cfg(feature = "postgres")]
            let res = diesel::insert_into(archive::table)
                .values(&model)
                .on_conflict(archive::hash)
                .do_nothing()
                .execute(&mut conn);

            res
        } else {
            diesel::insert_into(archive::table)
                .values(&model)
                .execute(&mut conn)
        };

        match result {
            Ok(0) => skipped += 1,
            Ok(_) => imported += 1,
            Err(e) => {
                eprintln!("Line {}: insert error: {}", line_num + 1, e);
                errors += 1;
            }
        }
    }

    eprintln!(
        "Import complete: {} imported, {} skipped, {} errors",
        imported, skipped, errors
    );
}

fn view(pool: &DbPool, hash: String) {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut conn = pool.get().expect("Failed to get connection");

    let archive: ArchiveModel = match archive::table
        .filter(archive::hash.eq(&hash))
        .select(ArchiveModel::as_select())
        .first(&mut conn)
    {
        Ok(a) => a,
        Err(diesel::NotFound) => {
            eprintln!("Error: Archive not found with hash: {}", hash);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: Failed to query archive: {}", e);
            std::process::exit(1);
        }
    };

    // Decompress the payload
    let mut decoder = GzDecoder::new(&archive.compressed_payload[..]);
    let mut decompressed = Vec::new();

    if let Err(e) = decoder.read_to_end(&mut decompressed) {
        eprintln!("Error: Failed to decompress archive: {}", e);
        std::process::exit(1);
    }

    // Try to pretty-print as JSON
    match serde_json::from_slice::<serde_json::Value>(&decompressed) {
        Ok(json) => {
            // Pretty print JSON
            if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                println!("{}", pretty);
            } else {
                // Fallback to raw if pretty-print fails
                println!("{}", String::from_utf8_lossy(&decompressed));
            }
        }
        Err(_) => {
            // Not valid JSON, output as raw text
            println!("{}", String::from_utf8_lossy(&decompressed));
        }
    }
}
