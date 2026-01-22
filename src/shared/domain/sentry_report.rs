use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentryReport {
    pub event_id: Option<String>,
    pub timestamp: Option<String>,
    pub platform: Option<String>,
    pub level: Option<String>,
    pub release: Option<String>,
    pub environment: Option<String>,
    pub sdk: Option<SentrySdk>,
    pub contexts: Option<SentryContexts>,
    pub tags: Option<HashMap<String, serde_json::Value>>,
    pub exception: Option<SentryException>,
    pub user: Option<serde_json::Value>,
    pub request: Option<serde_json::Value>,
    pub breadcrumbs: Option<serde_json::Value>,
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
    pub device: Option<SentryContext>,
    pub os: Option<SentryContext>,
    pub app: Option<SentryContext>,
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
    pub frames: Option<Vec<serde_json::Value>>,
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
}
