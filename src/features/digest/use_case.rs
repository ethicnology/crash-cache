use diesel::Connection;
use sha2::{Digest, Sha256};
use tracing::{error, info};

use crate::shared::compression::GzipCompressor;
use crate::shared::domain::{DomainError, QueueItem, SentryBreadcrumb, SentryReport};
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
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::ConnectionPool(format!("Connection pool error: {}", e)))?;
        let items = self.repos.queue.dequeue_batch(&mut conn, limit)?;
        let mut processed_count = 0u32;

        for item in items {
            match self.process_single_item(&item) {
                Ok(()) => {
                    processed_count += 1;
                }
                Err(DomainError::DuplicateEventId(event_id)) => {
                    info!(
                        archive_hash = %item.archive_hash,
                        event_id = %event_id,
                        "Duplicate event_id, skipping (already processed)"
                    );
                    let mut conn = self
                        .pool
                        .get()
                        .map_err(|e| DomainError::Database(e.to_string()))?;
                    self.repos.queue.remove(&mut conn, &item.archive_hash)?;
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

        let archive = self.repos.archive.find_by_hash(&mut conn, &item.archive_hash)?
            .ok_or_else(|| DomainError::NotFound(item.archive_hash.clone()))?;
        let payload = self.compressor.decompress(&archive.compressed_payload)?;
        drop(conn);
        let native_report = crate::shared::symbolication::process_payload(&payload, archive.project_id)?;
        let mut conn = self.pool.get().map_err(|error| DomainError::ConnectionPool(error.to_string()))?;
        conn.transaction(|conn| self.process_single_item_tx(conn, item, archive.project_id, &payload, native_report))
    }

    fn process_single_item_tx(
        &self,
        conn: &mut DbConnection,
        item: &QueueItem,
        project_id: i32,
        decompressed: &[u8],
        native_report: Option<SentryReport>,
    ) -> Result<(), DomainError> {

        // Try to parse as envelope first to extract session
        let session_id = self.extract_and_store_session(conn, decompressed, project_id)?;

        if let Some(envelope) = Envelope::parse(decompressed).filter(|envelope| !envelope.items.is_empty()) {
            crate::shared::persistence::rich_telemetry::store_envelope_context(conn, project_id, &item.archive_hash, &envelope)?;
            if native_report.is_none() && envelope.find_event_payload().is_none() {
                self.repos.queue.remove(conn, &item.archive_hash)?;
                return Ok(());
            }
        }

        // Try parsing as raw JSON first, then as envelope format
        let mut sentry_report: SentryReport = match native_report {
            Some(report) => report,
            None => self.parse_payload(decompressed)?,
        };

        sentry_report.resolve_exception_thread_stacks();

        // The event savepoint may roll back a duplicate without discarding newer
        // session updates carried by the same envelope.
        let result = conn.transaction(|conn| self.store_event(conn, item, project_id, session_id, &sentry_report));
        match result {
            Ok(()) | Err(DomainError::DuplicateEventId(_)) => {
                self.repos.queue.remove(conn, &item.archive_hash)?;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn store_event(
        &self,
        conn: &mut DbConnection,
        item: &QueueItem,
        project_id: i32,
        session_id: Option<i32>,
        sentry_report: &SentryReport,
    ) -> Result<(), DomainError> {

        let event_id = sentry_report
            .event_id
            .clone()
            .map(|id| uuid::Uuid::parse_str(&id).map(|id| id.simple().to_string()).unwrap_or(id))
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());

        let timestamp = self.parse_timestamp(&sentry_report.timestamp);

        let platform_id = self.get_or_create_unwrap(conn, &sentry_report.platform, |conn, v| {
            self.repos.platform.get_or_create(conn, v)
        })?;

        let environment_id =
            self.get_or_create_unwrap(conn, &sentry_report.environment, |conn, v| {
                self.repos.environment.get_or_create(conn, v)
            })?;

        let (os_name_id, os_version_id) = self.extract_os_info(conn, &sentry_report)?;
        let (manufacturer_id, brand_id, model_id, chipset_id, device_specs_id) =
            self.extract_device_info(conn, &sentry_report)?;
        let (locale_code_id, timezone_id, connection_type_id, orientation_id) =
            self.extract_locale_info(conn, &sentry_report)?;
        let (app_name_id, app_version_id, app_build_id) =
            self.extract_app_info(conn, &sentry_report)?;
        let user_id = self.extract_user_info(conn, &sentry_report)?;
        let (exception_type_id, exception_message_id, stacktrace_id, issue_id) =
            self.extract_exception_info(conn, sentry_report, project_id)?;

        let new_report = NewReport {
            event_id,
            archive_hash: item.archive_hash.clone(),
            timestamp,
            project_id,
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

        let report_id = self.repos.report.create(conn, new_report)?;
        self.extract_tags(conn, report_id, &sentry_report)?;
        self.extract_contexts(conn, report_id, &sentry_report)?;
        self.extract_breadcrumbs(conn, report_id, &sentry_report)?;

        Ok(())
    }

    /// Extract every `event.tags` K/V into the `report_tag` join table.
    /// Sentry tag spec: values are strings; coerce non-strings via to_string()
    /// and skip nulls.
    fn extract_tags(
        &self,
        conn: &mut DbConnection,
        report_id: i32,
        report: &SentryReport,
    ) -> Result<(), DomainError> {
        let map = match &report.tags {
            Some(m) => m,
            None => return Ok(()),
        };
        let mut pairs: Vec<(i32, i32)> = Vec::with_capacity(map.len());
        for (k, v) in map {
            let value_str = match v {
                serde_json::Value::Null => continue,
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let key_id = self.repos.tag_key.get_or_create(conn, k)?;
            let value_id = self.repos.tag_value.get_or_create(conn, &value_str)?;
            pairs.push((key_id, value_id));
        }
        self.repos.report_tag.insert_all(conn, report_id, &pairs)
    }

    /// Extract every CUSTOM context into the `report_context` join table.
    /// Built-in contexts (device/os/app/...) are already extracted via
    /// dedicated `unwrap_*` columns and skipped here. Only the flatten
    /// catch-all `SentryContexts.extra` is iterated. One row per top-level
    /// context name; value is the JSON-encoded payload (no flattening).
    fn extract_contexts(
        &self,
        conn: &mut DbConnection,
        report_id: i32,
        report: &SentryReport,
    ) -> Result<(), DomainError> {
        let contexts = match &report.contexts {
            Some(c) => c,
            None => return Ok(()),
        };
        let mut pairs: Vec<(i32, i32)> = Vec::with_capacity(contexts.extra.len());
        for (ctx_name, ctx_value) in &contexts.extra {
            if matches!(ctx_value, serde_json::Value::Null) {
                continue;
            }
            let value_str = serde_json::to_string(ctx_value)
                .unwrap_or_else(|_| ctx_value.to_string());
            let key_id = self.repos.context_key.get_or_create(conn, ctx_name)?;
            let value_id = self.repos.context_value.get_or_create(conn, &value_str)?;
            pairs.push((key_id, value_id));
        }
        self.repos.report_context.insert_all(conn, report_id, &pairs)
    }

    /// Persist every breadcrumb. The same physical breadcrumb is shared across
    /// every report whose capture timeline included it, so the row goes into
    /// `unwrap_breadcrumb` (hash-deduped, mirrors `unwrap_stacktrace`) and the
    /// per-report ordering lives in `report_breadcrumb` as a thin
    /// (report_id, seq, breadcrumb_id) join (mirrors `report_tag`).
    /// Hash domain: timestamp + category + type + level + data (canonical JSON).
    fn extract_breadcrumbs(
        &self,
        conn: &mut DbConnection,
        report_id: i32,
        report: &SentryReport,
    ) -> Result<(), DomainError> {
        let breadcrumbs = match &report.breadcrumbs {
            Some(b) => b,
            None => return Ok(()),
        };
        if breadcrumbs.is_empty() {
            return Ok(());
        }
        let mut breadcrumb_ids: Vec<i32> = Vec::with_capacity(breadcrumbs.len());
        for bc in breadcrumbs {
            let timestamp = bc.timestamp.as_ref().and_then(|ts| {
                chrono::DateTime::parse_from_rfc3339(ts)
                    .ok()
                    .map(|dt| dt.timestamp_millis())
            });
            let category_id = match bc.category.as_deref() {
                Some(v) if !v.is_empty() => {
                    Some(self.repos.breadcrumb_category.get_or_create(conn, v)?)
                }
                _ => None,
            };
            let type_id = match bc.breadcrumb_type.as_deref() {
                Some(v) if !v.is_empty() => {
                    Some(self.repos.breadcrumb_type.get_or_create(conn, v)?)
                }
                _ => None,
            };
            let level_id = match bc.level.as_deref() {
                Some(v) if !v.is_empty() => {
                    Some(self.repos.breadcrumb_level.get_or_create(conn, v)?)
                }
                _ => None,
            };
            let data = Self::merge_breadcrumb_data(bc);
            let hash = self.compute_hash(
                Self::breadcrumb_canonical_form(timestamp, bc, data.as_ref()).as_bytes(),
            );
            let breadcrumb_id = self.repos.breadcrumb.get_or_create(
                conn,
                &hash,
                timestamp,
                category_id,
                type_id,
                level_id,
                data,
            )?;
            breadcrumb_ids.push(breadcrumb_id);
        }
        self.repos
            .report_breadcrumb
            .insert_all(conn, report_id, &breadcrumb_ids)
    }

    /// Build the deterministic JSON string used as input to `compute_hash`.
    /// `serde_json::Value::Object` uses BTreeMap (`preserve_order` feature off),
    /// so nested object keys serialize alphabetically — the resulting string
    /// is stable across runs even when input key order differs.
    fn breadcrumb_canonical_form(
        timestamp: Option<i64>,
        bc: &SentryBreadcrumb,
        data: Option<&serde_json::Value>,
    ) -> String {
        let canon = serde_json::json!({
            "category": bc.category,
            "data": data,
            "level": bc.level,
            "timestamp": timestamp,
            "type": bc.breadcrumb_type,
        });
        serde_json::to_string(&canon).unwrap_or_default()
    }

    /// Merge `breadcrumb.data` (object) with `breadcrumb.message` (string) into
    /// a single JSONB column. Returns None when both absent so the column stays NULL.
    fn merge_breadcrumb_data(bc: &SentryBreadcrumb) -> Option<serde_json::Value> {
        match (bc.data.as_ref(), bc.message.as_ref()) {
            (None, None) => None,
            (Some(serde_json::Value::Object(obj)), Some(msg)) => {
                let mut merged = obj.clone();
                merged.insert(
                    "message".to_string(),
                    serde_json::Value::String(msg.clone()),
                );
                Some(serde_json::Value::Object(merged))
            }
            (Some(other), Some(msg)) => {
                let mut merged = serde_json::Map::new();
                merged.insert("data".to_string(), other.clone());
                merged.insert(
                    "message".to_string(),
                    serde_json::Value::String(msg.clone()),
                );
                Some(serde_json::Value::Object(merged))
            }
            (Some(data), None) => Some(data.clone()),
            (None, Some(msg)) => {
                let mut obj = serde_json::Map::new();
                obj.insert(
                    "message".to_string(),
                    serde_json::Value::String(msg.clone()),
                );
                Some(serde_json::Value::Object(obj))
            }
        }
    }

    /// Extract session from envelope and store it (with connection), returning the session_id if found
    fn extract_and_store_session(
        &self,
        conn: &mut DbConnection,
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

        let mut stored_id = None;
        let single_session = session_payloads.len() == 1;
        for session_data in session_payloads {
            let session = SentrySession::parse(session_data)
                .ok_or_else(|| DomainError::InvalidRequest("Invalid session payload".into()))?;
            let status_id = self.repos.session_status.get_or_create(conn, &session.status)?;
            let release_id = match &session.attrs.release {
                Some(value) => Some(self.repos.session_release.get_or_create(conn, value)?),
                None => None,
            };
            let environment_id = match &session.attrs.environment {
                Some(value) => Some(self.repos.session_environment.get_or_create(conn, value)?),
                None => None,
            };
            let new_session = NewSessionModel {
                project_id,
                sid: session.sid.clone(),
                init: i32::from(session.init),
                started_at: session.started.clone(),
                timestamp: session.timestamp.clone().unwrap_or_else(|| session.started.clone()),
                errors: session.errors,
                status_id,
                release_id,
                environment_id,
                distinct_id: session.distinct_id_hash(),
                sequence: session.sequence_decimal(),
                duration: session.duration,
                abnormal_mechanism: session.abnormal_mechanism.clone(),
            };
            let id = self.repos.session.upsert(conn, new_session)?;
            if single_session {
                stored_id = Some(id);
            }
        }
        // An envelope may contain unrelated sessions; do not invent an event association.
        Ok(stored_id)
    }

    fn get_or_create_unwrap<F>(
        &self,
        conn: &mut DbConnection,
        value: &Option<String>,
        get_or_create_fn: F,
    ) -> Result<Option<i32>, DomainError>
    where
        F: FnOnce(&mut DbConnection, &str) -> Result<i32, DomainError>,
    {
        match value {
            Some(v) if !v.is_empty() => {
                let id = get_or_create_fn(conn, v)?;
                Ok(Some(id))
            }
            _ => Ok(None),
        }
    }

    fn extract_os_info(
        &self,
        conn: &mut DbConnection,
        report: &SentryReport,
    ) -> Result<(Option<i32>, Option<i32>), DomainError> {
        let os = report.contexts.as_ref().and_then(|c| c.os.as_ref());

        let os_name_id = match os.and_then(|o| o.name.as_ref()) {
            Some(name) => Some(self.repos.os_name.get_or_create(conn, name)?),
            None => None,
        };

        let os_version_id = match os.and_then(|o| o.version.as_ref()) {
            Some(version) => Some(self.repos.os_version.get_or_create(conn, version)?),
            None => None,
        };

        Ok((os_name_id, os_version_id))
    }

    fn extract_device_info(
        &self,
        conn: &mut DbConnection,
        report: &SentryReport,
    ) -> Result<DeviceIds, DomainError> {
        let device = report.contexts.as_ref().and_then(|c| c.device.as_ref());

        let manufacturer_id = match device.and_then(|d| d.manufacturer.as_ref()) {
            Some(v) => Some(self.repos.manufacturer.get_or_create(conn, v)?),
            None => None,
        };

        let brand_id = match device.and_then(|d| d.brand.as_ref()) {
            Some(v) => Some(self.repos.brand.get_or_create(conn, v)?),
            None => None,
        };

        let model_id = match device.and_then(|d| d.model.as_ref()) {
            Some(v) => Some(self.repos.model.get_or_create(conn, v)?),
            None => None,
        };

        let chipset_id = match device.and_then(|d| d.chipset.as_ref()) {
            Some(v) => Some(self.repos.chipset.get_or_create(conn, v)?),
            None => None,
        };

        let device_specs_id = if let Some(d) = device {
            let archs_json = d
                .archs
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default());
            Some(self.repos.device_specs.get_or_create(
                conn,
                DeviceSpecsParams {
                    screen_width: d.screen_width_pixels,
                    screen_height: d.screen_height_pixels,
                    screen_density: d.screen_density,
                    screen_dpi: d.screen_dpi,
                    processor_count: d.processor_count,
                    memory_size: d.memory_size,
                    archs: archs_json,
                },
            )?)
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

    fn extract_locale_info(
        &self,
        conn: &mut DbConnection,
        report: &SentryReport,
    ) -> Result<LocaleIds, DomainError> {
        let device = report.contexts.as_ref().and_then(|c| c.device.as_ref());
        let culture = report.contexts.as_ref().and_then(|c| c.culture.as_ref());

        let locale_code_id = match culture
            .and_then(|c| c.locale.as_ref())
            .or_else(|| device.and_then(|d| d.locale.as_ref()))
        {
            Some(v) => Some(self.repos.locale_code.get_or_create(conn, v)?),
            None => None,
        };

        let timezone_id = match culture
            .and_then(|c| c.timezone.as_ref())
            .or_else(|| device.and_then(|d| d.timezone.as_ref()))
        {
            Some(v) => Some(self.repos.timezone.get_or_create(conn, v)?),
            None => None,
        };

        let connection_type_id = match device.and_then(|d| d.connection_type.as_ref()) {
            Some(v) => Some(self.repos.connection_type.get_or_create(conn, v)?),
            None => None,
        };

        let orientation_id = match device.and_then(|d| d.orientation.as_ref()) {
            Some(v) => Some(self.repos.orientation.get_or_create(conn, v)?),
            None => None,
        };

        Ok((
            locale_code_id,
            timezone_id,
            connection_type_id,
            orientation_id,
        ))
    }

    fn extract_app_info(
        &self,
        conn: &mut DbConnection,
        report: &SentryReport,
    ) -> Result<AppIds, DomainError> {
        let app = report.contexts.as_ref().and_then(|c| c.app.as_ref());

        let release_cache: std::cell::OnceCell<(Option<String>, Option<String>, Option<String>)> =
            std::cell::OnceCell::new();
        let get_release = || release_cache.get_or_init(|| Self::parse_release(&report.release));

        let app_name_value = app
            .and_then(|a| a.app_name.clone())
            .or_else(|| app.and_then(|a| a.app_identifier.clone()))
            .or_else(|| get_release().0.clone());

        let app_name_id = match app_name_value {
            Some(ref v) => Some(self.repos.app_name.get_or_create(conn, v)?),
            None => None,
        };

        let app_version_value = app
            .and_then(|a| a.app_version.clone())
            .or_else(|| get_release().1.clone());

        let app_version_id = match app_version_value {
            Some(ref v) => Some(self.repos.app_version.get_or_create(conn, v)?),
            None => None,
        };

        let app_build_value = app
            .and_then(|a| a.app_build.clone())
            .or_else(|| report.dist.clone())
            .or_else(|| get_release().2.clone());

        let app_build_id = match app_build_value {
            Some(ref v) => Some(self.repos.app_build.get_or_create(conn, v)?),
            None => None,
        };

        Ok((app_name_id, app_version_id, app_build_id))
    }

    fn extract_user_info(
        &self,
        conn: &mut DbConnection,
        report: &SentryReport,
    ) -> Result<Option<i32>, DomainError> {
        match report.user.as_ref().and_then(|u| u.id.as_ref()) {
            Some(user_id) => Ok(Some(self.repos.user.get_or_create(conn, &self.compute_hash(user_id.as_bytes()))?)),
            None => Ok(None),
        }
    }

    fn extract_exception_info(
        &self,
        conn: &mut DbConnection,
        report: &SentryReport,
        project_id: i32,
    ) -> Result<ExceptionIds, DomainError> {
        let exception = report
            .exception
            .as_ref()
            .and_then(|e| e.values.as_ref())
            .and_then(|v| v.first());

        let exception_type_id = match exception.and_then(|e| e.exception_type.as_ref()) {
            Some(v) => Some(self.repos.exception_type.get_or_create(conn, v)?),
            None => None,
        };

        let exception_message_id = match exception.and_then(|e| e.value.as_ref()) {
            Some(msg) => {
                let hash = self.compute_hash(msg.as_bytes());
                Some(
                    self.repos
                        .exception_message
                        .get_or_create(conn, &hash, msg)?,
                )
            }
            None => None,
        };

        if exception.is_none() && matches!(report.level.as_deref(), Some("info" | "debug")) {
            return Ok((None, None, None, None));
        }

        let all_frames = exception.and_then(|exception| exception.stacktrace.as_ref()).and_then(|trace| trace.frames.as_ref());
        let mut grouping_frames = report.extract_in_app_frames();
        if grouping_frames.is_empty() {
            grouping_frames = all_frames.map(|frames| frames.iter().collect()).unwrap_or_default();
        }
        let fingerprint_hash = if grouping_frames.is_empty() { None } else {
            let frames: Vec<serde_json::Value> = grouping_frames.iter().map(|frame| {
                serde_json::json!([frame.filename, frame.function, frame.lineno, frame.package,
                    if frame.function.is_none() { frame.extra.get("instruction_addr") } else { None }])
            }).collect();
            Some(self.compute_hash(serde_json::to_string(&frames).unwrap_or_default().as_bytes()))
        };
        let stacktrace_hash = all_frames.map(|frames| self.compute_hash(serde_json::to_string(frames).unwrap_or_default().as_bytes()));

        // Explicit SDK fingerprints override grouping, never event occurrence identity.
        // Default grouping is deliberately local, not a claim of Sentry algorithm parity.
        let default_group = fingerprint_hash.clone().map(|stack| self.compute_hash(
            serde_json::json!([report.platform, exception.and_then(|value| value.exception_type.as_ref()), stack]).to_string().as_bytes()
        )).unwrap_or_else(|| self.compute_hash(
            serde_json::json!({
                "platform": report.platform,
                "exception": exception.map(|value| (&value.exception_type, &value.value)),
                "message": report.unknown.get("message"),
                "logentry": report.unknown.get("logentry"),
            }).to_string().as_bytes()
        ));
        let fingerprints = report.unknown.get("fingerprint").and_then(|value| value.as_array());
        let values: Vec<String> = match fingerprints.filter(|values| !values.is_empty() && values.iter().all(|value| value.is_string())) {
            Some(values) => values.iter().map(|value| {
                let value = value.as_str().unwrap_or_default();
                if value.replace(' ', "") == "{{default}}" { default_group.clone() } else { value.to_owned() }
            }).collect(),
            None => vec![default_group],
        };
        let fingerprint_hash = Some(self.compute_hash(serde_json::json!([project_id, values]).to_string().as_bytes()));

        let issue_id = match &fingerprint_hash {
            Some(fp) => {
                let title = exception.and_then(|e| e.exception_type.as_ref()).cloned();
                Some(
                    self.repos
                        .issue
                        .get_or_create(conn, fp, exception_type_id, title)?,
                )
            }
            None => None,
        };

        let stacktrace_id = match (&stacktrace_hash, &exception) {
            (Some(hash), Some(exc)) => {
                let frames = exc
                    .stacktrace
                    .as_ref()
                    .and_then(|s| s.frames.as_ref())
                    .and_then(|f| serde_json::to_value(f).ok())
                    .unwrap_or(serde_json::Value::Array(vec![]));

                Some(self.repos.stacktrace.get_or_create(
                    conn,
                    hash,
                    fingerprint_hash.clone(),
                    frames,
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
                let mut report: SentryReport = serde_json::from_slice(event_payload)
                    .map_err(|e| DomainError::Serialization(format!("Invalid event JSON: {}", e)))?;
                if let (Some(header_id), Some(payload_id)) = (&envelope.header.event_id, &report.event_id) {
                    let normalize = |id: &str| uuid::Uuid::parse_str(id).map(|id| id.simple().to_string()).unwrap_or_else(|_| id.to_owned());
                    if normalize(header_id) != normalize(payload_id) {
                        return Err(DomainError::Serialization("Envelope and event IDs differ".to_owned()));
                    }
                }
                report.event_id = report.event_id.or(envelope.header.event_id);
                return Ok(report);
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

        // Get connection for error handling
        let mut conn = self
            .pool
            .get()
            .map_err(|e| DomainError::Database(e.to_string()))?;

        // Record the error
        self.repos
            .queue_error
            .record_error(&mut conn, &item.archive_hash, &err.to_string())?;

        // Remove from processing queue
        self.repos.queue.remove(&mut conn, &item.archive_hash)?;

        Ok(())
    }
}
