use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryReport {
    pub event_id: Option<String>,
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
    pub breadcrumbs: Option<serde_json::Value>,
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
    pub stacktrace: Option<SentryStacktrace>,
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
    pub fn extract_app_version(&self) -> Option<String> {
        if let Some(release) = &self.release {
            if let Some(version) = release.split('@').last() {
                return Some(version.to_string());
            }
            return Some(release.clone());
        }
        if let Some(contexts) = &self.contexts {
            if let Some(app) = &contexts.app {
                if let Some(version) = &app.app_version {
                    return Some(version.clone());
                }
            }
        }
        None
    }

    pub fn extract_error_info(&self) -> (Option<String>, Option<String>) {
        if let Some(exception) = &self.exception {
            if let Some(values) = &exception.values {
                if let Some(first) = values.first() {
                    return (first.exception_type.clone(), first.value.clone());
                }
            }
        }
        (None, None)
    }

    pub fn extract_sdk_info(&self) -> (Option<String>, Option<String>) {
        if let Some(sdk) = &self.sdk {
            return (sdk.name.clone(), sdk.version.clone());
        }
        (None, None)
    }

    pub fn extract_in_app_frames(&self) -> Vec<&SentryStacktraceFrame> {
        let mut frames = Vec::new();
        if let Some(exception) = &self.exception {
            if let Some(values) = &exception.values {
                for value in values {
                    if let Some(stacktrace) = &value.stacktrace {
                        if let Some(st_frames) = &stacktrace.frames {
                            for frame in st_frames {
                                if frame.in_app.unwrap_or(false) {
                                    frames.push(frame);
                                }
                            }
                        }
                    }
                }
            }
        }
        frames
    }
}
