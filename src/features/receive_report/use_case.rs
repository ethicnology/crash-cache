use sha2::{Digest, Sha256};

use crate::shared::compression::GzipCompressor;
use crate::shared::domain::{Archive, DomainError, ProcessingQueueItem};
use crate::shared::persistence::{ArchiveRepository, ProjectRepository, QueueRepository};

#[derive(Clone)]
pub struct IngestReportUseCase {
    archive_repo: ArchiveRepository,
    queue_repo: QueueRepository,
    project_repo: ProjectRepository,
    compressor: GzipCompressor,
}

impl IngestReportUseCase {
    pub fn new(
        archive_repo: ArchiveRepository,
        queue_repo: QueueRepository,
        project_repo: ProjectRepository,
        compressor: GzipCompressor,
    ) -> Self {
        Self {
            archive_repo,
            queue_repo,
            project_repo,
            compressor,
        }
    }

    pub fn execute(&self, project_id: i32, payload: &[u8]) -> Result<String, DomainError> {
        if !self.project_repo.exists(project_id)? {
            return Err(DomainError::ProjectNotFound(project_id));
        }

        let hash = self.compute_hash(payload);

        let archive_exists = self.archive_repo.exists(&hash)?;

        if !archive_exists {
            let compressed = self.compressor.compress(payload)?;
            let archive = Archive::new(hash.clone(), compressed, payload.len() as i32);
            self.archive_repo.save(&archive)?;

            let queue_item = ProcessingQueueItem::new(hash.clone());
            self.queue_repo.enqueue(&queue_item)?;
        }

        Ok(hash)
    }

    fn compute_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}
