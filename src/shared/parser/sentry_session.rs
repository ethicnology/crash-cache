use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Represents a Sentry session payload
#[derive(Debug, Clone, Deserialize)]
pub struct SentrySession {
    /// Unique session identifier
    pub sid: String,

    /// Pseudonymous installation or user identifier.
    pub did: Option<String>,

    /// Logical update clock; the full unsigned wire range is retained.
    pub seq: Option<u64>,

    /// Session duration in seconds.
    pub duration: Option<f64>,

    pub abnormal_mechanism: Option<String>,

    /// Whether this is the initial session update
    #[serde(default)]
    pub init: bool,

    /// When the session started (ISO 8601 timestamp)
    pub started: String,

    /// Current update timestamp (ISO 8601 timestamp)  
    pub timestamp: Option<String>,

    /// Number of errors in this session
    #[serde(default)]
    pub errors: i32,

    /// Session status: ok, crashed, abnormal, exited
    #[serde(default = "default_status")]
    pub status: String,

    /// Session attributes
    #[serde(default)]
    pub attrs: SessionAttrs,
}

fn default_status() -> String {
    "ok".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionAttrs {
    /// App release version (e.g., "com.example.app@1.0.0")
    pub release: Option<String>,

    /// Environment (e.g., "production", "staging")
    pub environment: Option<String>,
}

impl SentrySession {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let mut session: Self = serde_json::from_slice(data).ok()?;
        session.sid = uuid::Uuid::parse_str(&session.sid).ok()?.to_string();
        let started = DateTime::parse_from_rfc3339(&session.started).ok()?;
        let timestamp = match &session.timestamp {
            Some(value) => DateTime::parse_from_rfc3339(value).ok()?.with_timezone(&Utc),
            None => Utc::now(),
        };
        if session.errors < 0
            || session.duration.is_some_and(|value| !value.is_finite() || value < 0.0)
            || !matches!(session.status.as_str(), "ok" | "exited" | "crashed" | "abnormal" | "unhandled")
            || session.attrs.release.as_ref().is_none_or(|value| value.is_empty())
        {
            return None;
        }
        session.started = started.with_timezone(&Utc).to_rfc3339_opts(SecondsFormat::Nanos, true);
        session.timestamp = Some(timestamp.to_rfc3339_opts(SecondsFormat::Nanos, true));
        session.seq = Some(if session.init { 0 } else {
            session.seq.unwrap_or_else(|| timestamp.timestamp_millis().max(0) as u64)
        });
        if session.status == "crashed" {
            session.errors = session.errors.max(1);
        }
        Some(session)
    }

    /// Do not persist the supplied distinct identifier in the normalized session table.
    pub fn distinct_id_hash(&self) -> Option<String> {
        self.did.as_ref().map(|value| hex::encode(Sha256::digest(value.as_bytes())))
    }

    pub fn sequence_decimal(&self) -> String {
        self.seq.unwrap_or(0).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::SentrySession;

    #[test]
    fn session_rejects_invalid_protocol_values() {
        let base = serde_json::json!({
            "sid": "7c7b6585-f901-4351-bf8d-02711b721929",
            "started": "2026-09-26T10:00:00Z",
            "attrs": {"release": "game@1"}
        });
        for (field, value) in [
            ("errors", serde_json::json!(-1)),
            ("duration", serde_json::json!(-1)),
            ("seq", serde_json::json!(-1)),
            ("status", serde_json::json!("unknown")),
            ("started", serde_json::json!("not-a-date")),
            ("sid", serde_json::json!("not-a-uuid")),
        ] {
            let mut payload = base.clone();
            payload[field] = value;
            assert!(SentrySession::parse(&serde_json::to_vec(&payload).unwrap()).is_none(), "accepted invalid {field}");
        }
    }

    #[test]
    fn session_crash_counts_at_least_one_error() {
        let payload = br#"{"sid":"7c7b6585-f901-4351-bf8d-02711b721929","started":"2026-09-26T10:00:00Z","status":"crashed","errors":0,"attrs":{"release":"game@1"}}"#;
        assert_eq!(SentrySession::parse(payload).unwrap().errors, 1);
    }

    #[test]
    fn session_retains_protocol_identity_and_full_sequence_range() {
        let payload = br#"{"sid":"7c7b6585f9014351bf8d02711b721929","did":"installation-id","seq":18446744073709551615,"started":"2026-09-26T12:00:00+02:00","timestamp":"2026-09-26T10:01:00Z","duration":60.5,"status":"unhandled","attrs":{"release":"game@1"}}"#;
        let session = SentrySession::parse(payload).unwrap();
        assert_eq!(session.sid, "7c7b6585-f901-4351-bf8d-02711b721929");
        assert_eq!(session.sequence_decimal(), u64::MAX.to_string());
        assert_eq!(session.duration, Some(60.5));
        assert_eq!(session.started, "2026-09-26T10:00:00.000000000Z");
        assert_eq!(session.distinct_id_hash().unwrap().len(), 64);
        assert_ne!(session.distinct_id_hash().unwrap(), "installation-id");
    }

    #[test]
    fn session_initial_clock_is_zero_even_when_client_supplies_sequence() {
        let payload = br#"{"sid":"7c7b6585-f901-4351-bf8d-02711b721929","init":true,"seq":123,"started":"2026-09-26T10:00:00Z","attrs":{"release":"game@1"}}"#;
        assert_eq!(SentrySession::parse(payload).unwrap().sequence_decimal(), "0");
    }

}
