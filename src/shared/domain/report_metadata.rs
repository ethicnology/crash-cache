use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct ReportMetadata {
    pub id: Option<i32>,
    pub event_id: i32,
    pub app_version: Option<String>,
    pub platform: Option<String>,
    pub environment: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub sdk_name: Option<String>,
    pub sdk_version: Option<String>,
    pub processed_at: DateTime<Utc>,
}

impl ReportMetadata {
    pub fn new(event_id: i32) -> Self {
        Self {
            id: None,
            event_id,
            app_version: None,
            platform: None,
            environment: None,
            error_type: None,
            error_message: None,
            sdk_name: None,
            sdk_version: None,
            processed_at: Utc::now(),
        }
    }

    pub fn with_app_version(mut self, version: Option<String>) -> Self {
        self.app_version = version;
        self
    }

    pub fn with_platform(mut self, platform: Option<String>) -> Self {
        self.platform = platform;
        self
    }

    pub fn with_environment(mut self, env: Option<String>) -> Self {
        self.environment = env;
        self
    }

    pub fn with_error(mut self, error_type: Option<String>, error_message: Option<String>) -> Self {
        self.error_type = error_type;
        self.error_message = error_message;
        self
    }

    pub fn with_sdk(mut self, name: Option<String>, version: Option<String>) -> Self {
        self.sdk_name = name;
        self.sdk_version = version;
        self
    }
}
