use diesel::Connection;
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use crate::shared::compression::GzipCompressor;
use crate::shared::domain::{DomainError, QueueItem, SentryReport};
use crate::shared::parser::{Envelope, SentrySession};
use crate::shared::persistence::db::models::NewSessionModel;
use crate::shared::persistence::{
    DbConnection, DbPool, DeviceSpecsParams, NewReport, Repositories,
};

// Type aliases for complex return types
type DeviceIds = (
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);
type LocaleIds = (Option<i32>, Option<i32>, Option<i32>, Option<i32>);
type AppIds = (Option<i32>, Option<i32>, Option<i32>);
type ExceptionIds = (Option<i32>, Option<i32>, Option<i32>, Option<i32>);

#[derive(Clone)]
pub struct DigestReportUseCase {
    repos: Repositories,
    pool: DbPool,
    compressor: GzipCompressor,
}

impl DigestReportUseCase {
    pub fn new(repos: Repositories, pool: DbPool, compressor: GzipCompressor) -> Self {
        Self {
            repos,
            pool,
            compressor,
        }
    }

    pub fn process_batch(&self, limit: i32) -> Result<u32, DomainError> {
        let items = self.repos.queue.dequeue_batch(limit)?;
        let mut processed_count = 0u32;

        for item in items {
            match self.process_single_item(&item) {
                Ok(()) => {
                    processed_count += 1;
                    info!(archive_hash = %item.archive_hash, "Successfully processed report");
                }
                Err(DomainError::DuplicateEventId(event_id)) => {
                    info!(
                        archive_hash = %item.archive_hash,
                        event_id = %event_id,
                        "Duplicate event_id, skipping (already processed)"
                    );
                    self.repos.queue.remove(&item.archive_hash)?;
                }
                Err(e) => {
                    self.handle_failure(&item, e)?;
                }
            }
        }

        Ok(processed_count)
    }

    fn process_single_item(&self, item: &QueueItem) -> Result<(), DomainError> {
        // Get a connection and wrap everything in a transaction
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;

        conn.transaction(|conn| {
            self.process_single_item_tx(conn, item)
                .map_err(|_| diesel::result::Error::RollbackTransaction)
        })
        .map_err(|e| DomainError::Database(e.to_string()))
    }

    fn process_single_item_tx(
        &self,
        _conn: &mut DbConnection,
        item: &QueueItem,
    ) -> Result<(), DomainError> {
        let archive = self
            .repos
            .archive
            .find_by_hash(&item.archive_hash)?
            .ok_or_else(|| {
                DomainError::NotFound(format!("Archive {} not found", item.archive_hash))
            })?;

        let decompressed = self.compressor.decompress(&archive.compressed_payload)?;

        // Try to parse as envelope first to extract session
        let session_id = self.extract_and_store_session(&decompressed, archive.project_id)?;

        // Try parsing as raw JSON first, then as envelope format
        let sentry_report: SentryReport = self.parse_payload(&decompressed)?;

        let event_id = sentry_report
            .event_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let timestamp = self.parse_timestamp(&sentry_report.timestamp);

        let platform_id = self.get_or_create_unwrap(&sentry_report.platform, |v| {
            self.repos.platform.get_or_create(v)
        })?;

        let environment_id = self.get_or_create_unwrap(&sentry_report.environment, |v| {
            self.repos.environment.get_or_create(v)
        })?;

        let (os_name_id, os_version_id) = self.extract_os_info(&sentry_report)?;
        let (manufacturer_id, brand_id, model_id, chipset_id, device_specs_id) =
            self.extract_device_info(&sentry_report)?;
        let (locale_code_id, timezone_id, connection_type_id, orientation_id) =
            self.extract_locale_info(&sentry_report)?;
        let (app_name_id, app_version_id, app_build_id) = self.extract_app_info(&sentry_report)?;
        let user_id = self.extract_user_info(&sentry_report)?;
        let (exception_type_id, exception_message_id, stacktrace_id, issue_id) =
            self.extract_exception_info(&sentry_report)?;

        let new_report = NewReport {
            event_id,
            archive_hash: item.archive_hash.clone(),
            timestamp,
            project_id: archive.project_id,
            platform_id,
            environment_id,
            os_name_id,
            os_version_id,
            manufacturer_id,
            brand_id,
            model_id,
            chipset_id,
            device_specs_id,
            locale_code_id,
            timezone_id,
            connection_type_id,
            orientation_id,
            app_name_id,
            app_version_id,
            app_build_id,
            user_id,
            exception_type_id,
            exception_message_id,
            stacktrace_id,
            issue_id,
            session_id,
        };

        self.repos.report.create(new_report)?;
        self.repos.queue.remove(&item.archive_hash)?;

        Ok(())
    }

    /// Extract session from envelope and store it, returning the session_id if found
    fn extract_and_store_session(
        &self,
        decompressed: &[u8],
        project_id: i32,
    ) -> Result<Option<i32>, DomainError> {
        // Try to parse as envelope
        let envelope = match Envelope::parse(decompressed) {
            Some(env) => env,
            None => return Ok(None), // Not an envelope format, no session
        };

        // Find session payloads
        let session_payloads = envelope.find_session_payloads();
        if session_payloads.is_empty() {
            return Ok(None);
        }

        // Process the first (typically only) session
        let session_data = session_payloads[0];
        let session = match SentrySession::parse(session_data) {
            Some(s) => s,
            None => {
                warn!("Failed to parse session payload");
                return Ok(None);
            }
        };

        // Get or create status_id
        let status_id = self.repos.session_status.get_or_create(&session.status)?;

        // Get or create release_id (optional)
        let release_id = match &session.attrs.release {
            Some(r) => Some(self.repos.session_release.get_or_create(r)?),
            None => None,
        };

        // Get or create environment_id (optional)
        let environment_id = match &session.attrs.environment {
            Some(env) => Some(self.repos.session_environment.get_or_create(env)?),
            None => None,
        };

        let new_session = NewSessionModel {
            project_id,
            sid: session.sid.clone(),
            init: if session.init { 1 } else { 0 },
            started_at: session.started.clone(),
            timestamp: session
                .timestamp
                .clone()
                .unwrap_or_else(|| session.started.clone()),
            errors: session.errors,
            status_id,
            release_id,
            environment_id,
        };

        match self.repos.session.upsert(new_session) {
            Ok(session_id) => {
                debug!(sid = %session.sid, session_id = %session_id, status = %session.status, "Session stored during digest");
                Ok(Some(session_id))
            }
            Err(e) => {
                warn!(error = %e, sid = %session.sid, "Failed to store session during digest");
                Ok(None)
            }
        }
    }

    fn get_or_create_unwrap<F>(
        &self,
        value: &Option<String>,
        get_or_create_fn: F,
    ) -> Result<Option<i32>, DomainError>
    where
        F: FnOnce(&str) -> Result<i32, DomainError>,
    {
        match value {
            Some(v) if !v.is_empty() => {
                let id = get_or_create_fn(v)?;
                Ok(Some(id))
            }
            _ => Ok(None),
        }
    }

    fn extract_os_info(
        &self,
        report: &SentryReport,
    ) -> Result<(Option<i32>, Option<i32>), DomainError> {
        let os = report.contexts.as_ref().and_then(|c| c.os.as_ref());

        let os_name_id = match os.and_then(|o| o.name.as_ref()) {
            Some(name) => Some(self.repos.os_name.get_or_create(name)?),
            None => None,
        };

        let os_version_id = match os.and_then(|o| o.version.as_ref()) {
            Some(version) => Some(self.repos.os_version.get_or_create(version)?),
            None => None,
        };

        Ok((os_name_id, os_version_id))
    }

    fn extract_device_info(&self, report: &SentryReport) -> Result<DeviceIds, DomainError> {
        let device = report.contexts.as_ref().and_then(|c| c.device.as_ref());

        let manufacturer_id = match device.and_then(|d| d.manufacturer.as_ref()) {
            Some(v) => Some(self.repos.manufacturer.get_or_create(v)?),
            None => None,
        };

        let brand_id = match device.and_then(|d| d.brand.as_ref()) {
            Some(v) => Some(self.repos.brand.get_or_create(v)?),
            None => None,
        };

        let model_id = match device.and_then(|d| d.model.as_ref()) {
            Some(v) => Some(self.repos.model.get_or_create(v)?),
            None => None,
        };

        let chipset_id = match device.and_then(|d| d.chipset.as_ref()) {
            Some(v) => Some(self.repos.chipset.get_or_create(v)?),
            None => None,
        };

        let device_specs_id = if let Some(d) = device {
            let archs_json = d
                .archs
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default());
            Some(self.repos.device_specs.get_or_create(DeviceSpecsParams {
                screen_width: d.screen_width_pixels,
                screen_height: d.screen_height_pixels,
                screen_density: d.screen_density,
                screen_dpi: d.screen_dpi,
                processor_count: d.processor_count,
                memory_size: d.memory_size,
                archs: archs_json,
            })?)
        } else {
            None
        };

        Ok((
            manufacturer_id,
            brand_id,
            model_id,
            chipset_id,
            device_specs_id,
        ))
    }

    fn extract_locale_info(&self, report: &SentryReport) -> Result<LocaleIds, DomainError> {
        let device = report.contexts.as_ref().and_then(|c| c.device.as_ref());
        let culture = report.contexts.as_ref().and_then(|c| c.culture.as_ref());

        let locale_code_id = match culture
            .and_then(|c| c.locale.as_ref())
            .or_else(|| device.and_then(|d| d.locale.as_ref()))
        {
            Some(v) => Some(self.repos.locale_code.get_or_create(v)?),
            None => None,
        };

        let timezone_id = match culture
            .and_then(|c| c.timezone.as_ref())
            .or_else(|| device.and_then(|d| d.timezone.as_ref()))
        {
            Some(v) => Some(self.repos.timezone.get_or_create(v)?),
            None => None,
        };

        let connection_type_id = match device.and_then(|d| d.connection_type.as_ref()) {
            Some(v) => Some(self.repos.connection_type.get_or_create(v)?),
            None => None,
        };

        let orientation_id = match device.and_then(|d| d.orientation.as_ref()) {
            Some(v) => Some(self.repos.orientation.get_or_create(v)?),
            None => None,
        };

        Ok((
            locale_code_id,
            timezone_id,
            connection_type_id,
            orientation_id,
        ))
    }

    fn extract_app_info(&self, report: &SentryReport) -> Result<AppIds, DomainError> {
        let app = report.contexts.as_ref().and_then(|c| c.app.as_ref());

        let release_cache: std::cell::OnceCell<(Option<String>, Option<String>, Option<String>)> =
            std::cell::OnceCell::new();
        let get_release = || release_cache.get_or_init(|| Self::parse_release(&report.release));

        let app_name_value = app
            .and_then(|a| a.app_name.clone())
            .or_else(|| app.and_then(|a| a.app_identifier.clone()))
            .or_else(|| get_release().0.clone());

        let app_name_id = match app_name_value {
            Some(ref v) => Some(self.repos.app_name.get_or_create(v)?),
            None => None,
        };

        let app_version_value = app
            .and_then(|a| a.app_version.clone())
            .or_else(|| get_release().1.clone());

        let app_version_id = match app_version_value {
            Some(ref v) => Some(self.repos.app_version.get_or_create(v)?),
            None => None,
        };

        let app_build_value = app
            .and_then(|a| a.app_build.clone())
            .or_else(|| report.dist.clone())
            .or_else(|| get_release().2.clone());

        let app_build_id = match app_build_value {
            Some(ref v) => Some(self.repos.app_build.get_or_create(v)?),
            None => None,
        };

        Ok((app_name_id, app_version_id, app_build_id))
    }

    fn parse_release(release: &Option<String>) -> (Option<String>, Option<String>, Option<String>) {
        let release_str = match release {
            Some(r) if !r.is_empty() => r,
            _ => return (None, None, None),
        };

        let (identifier, version_build) = match release_str.split_once('@') {
            Some((id, rest)) => (Some(id.to_string()), rest),
            None => return (None, None, None),
        };

        let (version, build) = match version_build.split_once('+') {
            Some((v, b)) => (Some(v.to_string()), Some(b.to_string())),
            None => (Some(version_build.to_string()), None),
        };

        (identifier, version, build)
    }

    fn extract_user_info(&self, report: &SentryReport) -> Result<Option<i32>, DomainError> {
        match report.user.as_ref().and_then(|u| u.id.as_ref()) {
            Some(user_id) => Ok(Some(self.repos.user.get_or_create(user_id)?)),
            None => Ok(None),
        }
    }

    fn extract_exception_info(&self, report: &SentryReport) -> Result<ExceptionIds, DomainError> {
        let exception = report
            .exception
            .as_ref()
            .and_then(|e| e.values.as_ref())
            .and_then(|v| v.first());

        let exception_type_id = match exception.and_then(|e| e.exception_type.as_ref()) {
            Some(v) => Some(self.repos.exception_type.get_or_create(v)?),
            None => None,
        };

        let exception_message_id = match exception.and_then(|e| e.value.as_ref()) {
            Some(msg) => {
                let hash = self.compute_hash(msg.as_bytes());
                Some(self.repos.exception_message.get_or_create(&hash, msg)?)
            }
            None => None,
        };

        let in_app_frames = report.extract_in_app_frames();
        let (fingerprint_hash, stacktrace_hash) = if !in_app_frames.is_empty() {
            let fingerprint_data = in_app_frames
                .iter()
                .map(|f| {
                    format!(
                        "{}:{}:{}",
                        f.filename.as_deref().unwrap_or(""),
                        f.function.as_deref().unwrap_or(""),
                        f.lineno.unwrap_or(0)
                    )
                })
                .collect::<Vec<_>>()
                .join("|");
            let fingerprint = self.compute_hash(fingerprint_data.as_bytes());

            let all_frames = exception
                .and_then(|e| e.stacktrace.as_ref())
                .and_then(|s| s.frames.as_ref());

            let stacktrace_hash = all_frames.map(|frames| {
                let frames_json = serde_json::to_string(frames).unwrap_or_default();
                self.compute_hash(frames_json.as_bytes())
            });

            (Some(fingerprint), stacktrace_hash)
        } else {
            (None, None)
        };

        let issue_id = match &fingerprint_hash {
            Some(fp) => {
                let title = exception.and_then(|e| e.exception_type.as_ref()).cloned();
                Some(
                    self.repos
                        .issue
                        .get_or_create(fp, exception_type_id, title)?,
                )
            }
            None => None,
        };

        let stacktrace_id = match (&stacktrace_hash, &exception) {
            (Some(hash), Some(exc)) => {
                let frames_json = exc
                    .stacktrace
                    .as_ref()
                    .and_then(|s| s.frames.as_ref())
                    .map(|f| serde_json::to_string(f).unwrap_or_default())
                    .unwrap_or_default();

                Some(self.repos.stacktrace.get_or_create(
                    hash,
                    fingerprint_hash.clone(),
                    &frames_json,
                )?)
            }
            _ => None,
        };

        Ok((
            exception_type_id,
            exception_message_id,
            stacktrace_id,
            issue_id,
        ))
    }

    fn compute_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Parse payload as either raw JSON or Sentry envelope format
    fn parse_payload(&self, data: &[u8]) -> Result<SentryReport, DomainError> {
        // Try raw JSON first (from /store endpoint)
        if let Ok(report) = serde_json::from_slice::<SentryReport>(data) {
            return Ok(report);
        }

        // Try envelope format (from /envelope endpoint)
        if let Some(envelope) = Envelope::parse(data) {
            if let Some(event_payload) = envelope.find_event_payload() {
                return serde_json::from_slice(event_payload)
                    .map_err(|e| DomainError::Serialization(format!("Invalid event JSON: {}", e)));
            }
            return Err(DomainError::Serialization(
                "No event found in envelope".to_string(),
            ));
        }

        Err(DomainError::Serialization(
            "Unable to parse payload as JSON or envelope".to_string(),
        ))
    }

    fn parse_timestamp(&self, timestamp: &Option<String>) -> i64 {
        timestamp
            .as_ref()
            .and_then(|ts| {
                chrono::DateTime::parse_from_rfc3339(ts)
                    .ok()
                    .map(|dt| dt.timestamp())
            })
            .unwrap_or_else(|| chrono::Utc::now().timestamp())
    }

    fn handle_failure(&self, item: &QueueItem, err: DomainError) -> Result<(), DomainError> {
        error!(
            archive_hash = %item.archive_hash,
            error = %err,
            "Failed to process report, moving to error queue"
        );

        // Record the error
        self.repos
            .queue_error
            .record_error(&item.archive_hash, &err.to_string())?;

        // Remove from processing queue
        self.repos.queue.remove(&item.archive_hash)?;

        Ok(())
    }
}
