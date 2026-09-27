use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryReport {
    pub event_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_timestamp")]
    pub timestamp: Option<String>,
    pub platform: Option<String>,
    pub level: Option<String>,
    pub release: Option<String>,
    pub dist: Option<String>,
    pub environment: Option<String>,
    pub sdk: Option<SentrySdk>,
    pub contexts: Option<SentryContexts>,
    pub tags: Option<HashMap<String, serde_json::Value>>,
    pub exception: Option<SentryException>,
    pub user: Option<SentryUser>,
    pub request: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "deserialize_breadcrumbs")]
    pub breadcrumbs: Option<Vec<SentryBreadcrumb>>,
    #[serde(flatten)]
    pub unknown: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryBreadcrumb {
    #[serde(default, deserialize_with = "deserialize_timestamp")]
    pub timestamp: Option<String>,
    pub category: Option<String>,
    #[serde(rename = "type")]
    pub breadcrumb_type: Option<String>,
    pub level: Option<String>,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryUser {
    pub id: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentrySdk {
    pub name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryContexts {
    pub device: Option<SentryDeviceContext>,
    pub os: Option<SentryOsContext>,
    pub app: Option<SentryAppContext>,
    pub culture: Option<SentryCultureContext>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryContext {
    pub name: Option<String>,
    pub version: Option<String>,
    pub app_version: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryDeviceContext {
    pub manufacturer: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub chipset: Option<String>,
    pub screen_width_pixels: Option<i32>,
    pub screen_height_pixels: Option<i32>,
    pub screen_density: Option<f32>,
    pub screen_dpi: Option<i32>,
    pub processor_count: Option<i32>,
    pub memory_size: Option<i64>,
    pub archs: Option<Vec<String>>,
    pub connection_type: Option<String>,
    pub orientation: Option<String>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryOsContext {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryAppContext {
    pub app_name: Option<String>,
    pub app_version: Option<String>,
    pub app_build: Option<String>,
    pub app_identifier: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryCultureContext {
    pub locale: Option<String>,
    pub timezone: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryException {
    pub values: Option<Vec<SentryExceptionValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryExceptionValue {
    #[serde(rename = "type")]
    pub exception_type: Option<String>,
    pub value: Option<String>,
    pub thread_id: Option<SentryThreadId>,
    pub stacktrace: Option<SentryStacktrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SentryThreadId {
    Number(u64),
    Text(String),
}

impl SentryThreadId {
    fn key(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.parse::<u64>().map(|number| number.to_string()).unwrap_or_else(|_| value.clone()),
        }
    }
}

#[derive(Deserialize)]
struct SentryThreads {
    values: Vec<SentryThread>,
}

#[derive(Deserialize)]
struct SentryThread {
    id: SentryThreadId,
    stacktrace: Option<SentryStacktrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryStacktrace {
    pub frames: Option<Vec<SentryStacktraceFrame>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryStacktraceFrame {
    pub filename: Option<String>,
    pub function: Option<String>,
    pub lineno: Option<i32>,
    pub colno: Option<i32>,
    pub abs_path: Option<String>,
    pub in_app: Option<bool>,
    pub platform: Option<String>,
    #[serde(rename = "package")]
    pub package: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl SentryReport {
    /// Android SDKs may keep exception frames under the associated thread.
    /// Resolve only an explicit, unambiguous ID; an exception's own stack wins.
    pub fn resolve_exception_thread_stacks(&mut self) {
        let Some(threads) = self.unknown.get("threads").and_then(|value| {
            serde_json::from_value::<SentryThreads>(value.clone()).ok()
        }) else { return; };
        let Some(exceptions) = self.exception.as_mut().and_then(|value| value.values.as_mut()) else { return; };
        let mut stacks: HashMap<String, Option<&SentryStacktrace>> = HashMap::new();
        for thread in &threads.values {
            stacks.entry(thread.id.key())
                .and_modify(|stack| *stack = None)
                .or_insert(thread.stacktrace.as_ref());
        }
        for exception in exceptions {
            if exception.stacktrace.is_some() { continue; }
            if let Some(stack) = exception.thread_id.as_ref()
                .and_then(|id| stacks.get(&id.key())).and_then(|stack| *stack) {
                exception.stacktrace = Some(stack.clone());
            }
        }
    }

    pub fn extract_app_version(&self) -> Option<String> {
        if let Some(release) = &self.release {
            if let Some(version) = release.split('@').next_back() {
                return Some(version.to_string());
            }
            return Some(release.clone());
        }
        if let Some(version) = self
            .contexts
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .and_then(|a| a.app_version.clone())
        {
            return Some(version);
        }
        None
    }

    pub fn extract_error_info(&self) -> (Option<String>, Option<String>) {
        if let Some(first) = self
            .exception
            .as_ref()
            .and_then(|e| e.values.as_ref())
            .and_then(|v| v.first())
        {
            return (first.exception_type.clone(), first.value.clone());
        }
        (None, None)
    }

    pub fn extract_sdk_info(&self) -> (Option<String>, Option<String>) {
        self.sdk
            .as_ref()
            .map(|sdk| (sdk.name.clone(), sdk.version.clone()))
            .unwrap_or((None, None))
    }

    pub fn extract_in_app_frames(&self) -> Vec<&SentryStacktraceFrame> {
        let mut frames = Vec::new();
        if let Some(values) = self.exception.as_ref().and_then(|e| e.values.as_ref()) {
            for value in values {
                if let Some(st_frames) = value.stacktrace.as_ref().and_then(|s| s.frames.as_ref()) {
                    for frame in st_frames {
                        if frame.in_app.unwrap_or(false) {
                            frames.push(frame);
                        }
                    }
                }
            }
        }
        frames
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::SentryReport;

    #[test]
    fn exception_thread_stacks_require_an_unambiguous_matching_id() {
        let frame = serde_json::json!({"filename":"probe.gd","vars":{"piece_count":12}});
        for (threads, expected) in [
            (serde_json::json!([{"id":7,"stacktrace":{"frames":[frame.clone()]}}]), true),
            (serde_json::json!([{"id":"7","stacktrace":{"frames":[frame.clone()]}}]), true),
            (serde_json::json!([{"id":8,"stacktrace":{"frames":[frame.clone()]}}]), false),
            (serde_json::json!([{"id":7,"stacktrace":{"frames":[frame.clone()]}},{"id":"7","stacktrace":{"frames":[frame.clone()]}}]), false),
        ] {
            let mut report: SentryReport = serde_json::from_value(serde_json::json!({
                "exception":{"values":[{"type":"Failure","thread_id":7}]},
                "threads":{"values":threads}
            })).unwrap();
            report.resolve_exception_thread_stacks();
            let exceptions = report.exception.unwrap().values.unwrap();
            let stack = &exceptions[0].stacktrace;
            assert_eq!(stack.is_some(), expected);
            if let Some(stack) = stack {
                assert_eq!(stack.frames.as_ref().unwrap()[0].extra["vars"]["piece_count"],12);
            }
        }
    }

    #[test]
    fn exception_thread_stacks_preserve_explicit_stack_and_ignore_missing_id() {
        for exception in [
            serde_json::json!({"type":"Failure","thread_id":7,"stacktrace":{"frames":[]}}),
            serde_json::json!({"type":"Failure"}),
        ] {
            let mut report: SentryReport = serde_json::from_value(serde_json::json!({
                "exception":{"values":[exception.clone()]},
                "threads":{"values":[{"id":7,"stacktrace":{"frames":[{"filename":"wrong.gd"}]}}]}
            })).unwrap();
            report.resolve_exception_thread_stacks();
            let saved = serde_json::to_value(report.exception.unwrap().values.unwrap()[0].clone()).unwrap();
            assert_eq!(saved["stacktrace"],exception.get("stacktrace").cloned().unwrap_or(serde_json::Value::Null));
        }
    }

    #[test]
    fn accepts_numeric_event_and_breadcrumb_timestamps() {
        let report: SentryReport = serde_json::from_value(serde_json::json!({
            "timestamp": 1770000000.125,
            "breadcrumbs": {"values": [{"timestamp": 1770000000.25, "message": "started"}]}
        })).unwrap();
        assert_eq!(report.timestamp.as_deref(), Some("2026-02-02T02:40:00.125+00:00"));
        assert_eq!(report.breadcrumbs.unwrap()[0].timestamp.as_deref(), Some("2026-02-02T02:40:00.250+00:00"));
    }

    #[test]
    fn accepts_flat_breadcrumb_array() {
        let report: SentryReport = serde_json::from_value(serde_json::json!({
            "breadcrumbs": [{"timestamp": "2026-01-01T00:00:00Z"}]
        })).unwrap();
        assert_eq!(report.breadcrumbs.unwrap().len(), 1);
    }
}

fn deserialize_timestamp<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(serde_json::Value::String(value)) => Ok(Some(value)),
        Some(serde_json::Value::Number(value)) => {
            let seconds = value.as_f64().ok_or_else(|| serde::de::Error::custom("Invalid timestamp"))?;
            let milliseconds = seconds * 1000.0;
            if !milliseconds.is_finite() || milliseconds < i64::MIN as f64 || milliseconds >= i64::MAX as f64 {
                return Err(serde::de::Error::custom("Timestamp out of range"));
            }
            chrono::DateTime::from_timestamp_millis(milliseconds.round() as i64)
                .map(|value| Some(value.to_rfc3339_opts(chrono::SecondsFormat::Millis, false)))
                .ok_or_else(|| serde::de::Error::custom("Timestamp out of range"))
        }
        _ => Err(serde::de::Error::custom("Timestamp must be a string or number")),
    }
}

fn deserialize_breadcrumbs<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<SentryBreadcrumb>>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Breadcrumbs {
        Array(Vec<SentryBreadcrumb>),
        Wrapped { values: Vec<SentryBreadcrumb> },
    }
    Ok(Option::<Breadcrumbs>::deserialize(deserializer)?.map(|value| match value {
        Breadcrumbs::Array(values) | Breadcrumbs::Wrapped { values } => values,
    }))
}
