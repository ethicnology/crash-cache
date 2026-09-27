//! Queryable log records and attachment metadata; payload bytes stay in archives.
use diesel::{RunQueryDsl, sql_query, sql_types::{BigInt, Double, Integer, Jsonb, Nullable, Text}};
use crate::shared::{domain::DomainError, parser::{Envelope, sentry_log::parse_logs}};
use super::DbConnection;

pub fn store_envelope_context(conn: &mut DbConnection, project_id: i32, archive_hash: &str, envelope: &Envelope) -> Result<(), DomainError> {
    for (item_index, log_index, log) in parse_logs(envelope)? {
        sql_query("INSERT INTO telemetry_log (project_id,archive_hash,item_index,log_index,timestamp,trace_id,span_id,level,body,severity_number,attributes) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (archive_hash,item_index,log_index) DO NOTHING")
            .bind::<Integer,_>(project_id).bind::<Text,_>(archive_hash).bind::<Integer,_>(item_index as i32).bind::<Integer,_>(log_index as i32)
            .bind::<Double,_>(log.timestamp).bind::<Nullable<Text>,_>(log.trace_id)
            .bind::<Nullable<Text>,_>(log.span_id).bind::<Text,_>(log.level)
            .bind::<Text,_>(log.body).bind::<Nullable<Integer>,_>(log.severity_number)
            .bind::<Jsonb,_>(log.attributes).execute(conn)?;
    }
    let event_id = envelope.header.event_id.clone().or_else(|| envelope.find_event_payload()
        .and_then(|payload| serde_json::from_slice::<serde_json::Value>(payload).ok())
        .and_then(|event| event.get("event_id").and_then(serde_json::Value::as_str).map(str::to_owned)))
        .and_then(|id| uuid::Uuid::parse_str(&id).ok()).map(|id| id.simple().to_string());
    for (index, item) in envelope.items.iter().enumerate().filter(|(_,item)| item.header.item_type == "attachment") {
        let filename = item.header.extra.get("filename").and_then(serde_json::Value::as_str);
        let attachment_type = item.header.extra.get("attachment_type").and_then(serde_json::Value::as_str);
        sql_query("INSERT INTO attachment_metadata (project_id,archive_hash,event_id,item_index,filename,attachment_type,content_type,size_bytes) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (archive_hash,item_index) DO NOTHING")
            .bind::<Integer,_>(project_id).bind::<Text,_>(archive_hash).bind::<Nullable<Text>,_>(event_id.as_deref())
            .bind::<Integer,_>(i32::try_from(index).map_err(|_| DomainError::InvalidRequest("Too many attachments".into()))?)
            .bind::<Nullable<Text>,_>(filename).bind::<Nullable<Text>,_>(attachment_type)
            .bind::<Nullable<Text>,_>(item.header.content_type.as_deref()).bind::<BigInt,_>(item.payload.len() as i64).execute(conn)?;
    }
    Ok(())
}
