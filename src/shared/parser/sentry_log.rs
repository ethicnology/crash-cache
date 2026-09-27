//! Sentry structured logs: https://develop.sentry.dev/sdk/telemetry/logs/.
//! Dart SDK 9.30.1 uses a `log` item containing {"items": [...]}.
use serde::Deserialize;
use serde_json::Value;
use crate::shared::{domain::DomainError, parser::Envelope};

#[derive(Debug, Deserialize)]
pub struct SentryLog {
    pub timestamp: f64,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub level: String,
    pub body: String,
    pub severity_number: Option<i32>,
    #[serde(default = "empty_attributes")]
    pub attributes: Value,
}

fn empty_attributes() -> Value {
    serde_json::json!({})
}

fn invalid() -> DomainError {
    DomainError::InvalidRequest("Invalid structured log batch".into())
}

/// Log entries have no occurrence UUID. Preserve envelope positions; do not
/// deduplicate legitimate identical records or use shared traces as identity.
pub fn parse_logs(envelope: &Envelope) -> Result<Vec<(usize, usize, SentryLog)>, DomainError> {
    let mut logs = Vec::new();
    for (item_index, item) in envelope.items.iter().enumerate().filter(|(_,item)| item.header.item_type == "log") {
        let value: Value = serde_json::from_slice(&item.payload).map_err(|_| invalid())?;
        let entries = value.get("items").and_then(Value::as_array).ok_or_else(invalid)?;
        if logs.len().saturating_add(entries.len()) > 10_000 { return Err(invalid()); }
        if let Some(count) = item.header.extra.get("item_count") {
            if count.as_u64() != Some(entries.len() as u64) { return Err(invalid()); }
        }
        for (log_index, entry) in entries.iter().enumerate() {
            let log: SentryLog = serde_json::from_value(entry.clone()).map_err(|_| invalid())?;
            if !log.timestamp.is_finite() || !(0.0..253402300800.0).contains(&log.timestamp)
                || !matches!(log.level.as_str(), "trace" | "debug" | "info" | "warn" | "error" | "fatal")
                || !log.attributes.is_object()
                || log.severity_number.is_some_and(|value| !(1..=24).contains(&value))
                || log.trace_id.is_none() || !valid_hex(log.trace_id.as_deref(), 32) || !valid_hex(log.span_id.as_deref(), 16)
            { return Err(invalid()); }
            for attribute in log.attributes.as_object().ok_or_else(invalid)?.values() {
                let value = attribute.get("value").ok_or_else(invalid)?;
                let valid = match attribute.get("type").and_then(Value::as_str) {
                    Some("string") => value.is_string(),
                    Some("boolean") => value.is_boolean(),
                    Some("integer") => value.is_i64() || value.is_u64(),
                    Some("double") => value.is_number(),
                    Some("array") => value.as_array().is_some_and(|values| {
                        values.iter().all(Value::is_string) || values.iter().all(Value::is_boolean)
                            || values.iter().all(Value::is_number)
                    }),
                    _ => false,
                };
                if !valid { return Err(invalid()); }
            }
            logs.push((item_index, log_index, log));
        }
    }
    Ok(logs)
}

fn valid_hex(value: Option<&str>, length: usize) -> bool {
    value.is_none_or(|value| value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn envelope(logs: Value) -> Envelope {
        Envelope::parse(format!("{{}}\n{{\"type\":\"log\"}}\n{logs}").as_bytes()).unwrap()
    }
    fn log() -> Value {
        serde_json::json!({"timestamp":1790424000.125,"trace_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","level":"info","body":"loaded","attributes":{"layer":{"type":"string","value":"flutter"}},"severity_number":9})
    }
    #[test]
    fn structured_logs_preserve_batch_multiplicity_and_retry_identity() {
        let batch = envelope(serde_json::json!({"items":[log(),log()]}));
        let first = parse_logs(&batch).unwrap();
        assert_eq!(first.len(),2);
        assert_ne!(first[0].1, first[1].1);
        assert_eq!(first[0].0, parse_logs(&batch).unwrap()[0].0);
        assert_eq!(first[0].2.attributes["layer"]["value"],"flutter");
    }
    #[test]
    fn structured_logs_accept_optional_attributes_but_require_trace() {
        let mut value = log();
        value.as_object_mut().unwrap().remove("attributes");
        let parsed = parse_logs(&envelope(serde_json::json!({"items":[value.clone()]}))).unwrap();
        assert_eq!(parsed[0].2.attributes, serde_json::json!({}));
        value.as_object_mut().unwrap().remove("trace_id");
        assert!(parse_logs(&envelope(serde_json::json!({"items":[value]}))).is_err());
    }
    #[test]
    fn structured_logs_reject_malformed_batches_and_attributes() {
        assert!(parse_logs(&envelope(serde_json::json!({"items":[{}]}))).is_err());
        let mut value = log(); value["attributes"]["layer"]["value"] = serde_json::json!(42);
        assert!(parse_logs(&envelope(serde_json::json!({"items":[value]}))).is_err());
        let mut value = log(); value["timestamp"] = serde_json::json!(1e99);
        assert!(parse_logs(&envelope(serde_json::json!({"items":[value]}))).is_err());
    }
}
