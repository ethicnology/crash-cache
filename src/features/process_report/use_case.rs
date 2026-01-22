use tracing::{error, info, warn};

use crate::shared::compression::GzipCompressor;
use crate::shared::domain::{DomainError, ProcessingQueueItem, ReportMetadata, SentryReport};
use crate::shared::persistence::{
    ArchiveRepository, EventRepository, QueueRepository, ReportMetadataRepository,
};

const MAX_RETRIES: i32 = 3;
const BACKOFF_BASE_SECONDS: i64 = 30;

#[derive(Clone)]
pub struct ProcessReportUseCase {
    archive_repo: ArchiveRepository,
    event_repo: EventRepository,
    queue_repo: QueueRepository,
    metadata_repo: ReportMetadataRepository,
    compressor: GzipCompressor,
}

impl ProcessReportUseCase {
    pub fn new(
        archive_repo: ArchiveRepository,
        event_repo: EventRepository,
        queue_repo: QueueRepository,
        metadata_repo: ReportMetadataRepository,
        compressor: GzipCompressor,
    ) -> Self {
        Self {
            archive_repo,
            event_repo,
            queue_repo,
            metadata_repo,
            compressor,
        }
    }

    pub fn process_batch(&self, limit: i32) -> Result<u32, DomainError> {
        let items = self.queue_repo.dequeue_batch(limit)?;
        let mut processed_count = 0u32;

        for item in items {
            match self.process_single_item(&item) {
                Ok(()) => {
                    processed_count += 1;
                    info!(event_id = item.event_id, "Successfully processed report event");
                }
                Err(e) => {
                    self.handle_failure(item, e)?;
                }
            }
        }

        Ok(processed_count)
    }

    fn process_single_item(&self, item: &ProcessingQueueItem) -> Result<(), DomainError> {
        let event = self
            .event_repo
            .find_by_id(item.event_id)?
            .ok_or_else(|| DomainError::NotFound(format!("Event {} not found", item.event_id)))?;

        let archive = self
            .archive_repo
            .find_by_hash(&event.archive_hash)?
            .ok_or_else(|| {
                DomainError::NotFound(format!("Archive {} not found", event.archive_hash))
            })?;

        let decompressed = self.compressor.decompress(&archive.compressed_payload)?;

        let sentry_report: SentryReport = serde_json::from_slice(&decompressed)
            .map_err(|e| DomainError::Serialization(e.to_string()))?;

        let app_version = sentry_report.extract_app_version();
        let (error_type, error_message) = sentry_report.extract_error_info();
        let (sdk_name, sdk_version) = sentry_report.extract_sdk_info();

        let metadata = ReportMetadata::new(item.event_id)
            .with_app_version(app_version)
            .with_platform(sentry_report.platform.clone())
            .with_environment(sentry_report.environment.clone())
            .with_error(error_type, error_message)
            .with_sdk(sdk_name, sdk_version);

        self.metadata_repo.save(&metadata)?;
        self.queue_repo.remove(item.event_id)?;
        self.event_repo.mark_processed(item.event_id)?;

        Ok(())
    }

    fn handle_failure(
        &self,
        mut item: ProcessingQueueItem,
        error: DomainError,
    ) -> Result<(), DomainError> {
        error!(
            event_id = item.event_id,
            error = %error,
            retry_count = item.retry_count,
            "Failed to process report event"
        );

        if item.retry_count >= MAX_RETRIES {
            warn!(
                event_id = item.event_id,
                "Max retries exceeded, removing from queue"
            );
            self.queue_repo.remove(item.event_id)?;
            return Err(DomainError::MaxRetriesExceeded(item.event_id));
        }

        let backoff = BACKOFF_BASE_SECONDS * (1 << item.retry_count);
        item.increment_retry(error.to_string(), backoff);
        self.queue_repo.update_retry(&item)?;

        Ok(())
    }
}
