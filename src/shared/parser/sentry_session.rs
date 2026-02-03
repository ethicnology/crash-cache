use serde::Deserialize;

/// Represents a Sentry session payload
#[derive(Debug, Clone, Deserialize)]
pub struct SentrySession {
    /// Unique session identifier
    pub sid: String,

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
        serde_json::from_slice(data).ok()
    }
}
