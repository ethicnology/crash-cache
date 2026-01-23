use crate::shared::domain::{Archive, DomainError, ProcessingQueueItem};
use crate::shared::persistence::{ArchiveRepository, ProjectRepository, QueueRepository};

#[derive(Clone)]
pub struct IngestReportUseCase {
    archive_repo: ArchiveRepository,
    queue_repo: QueueRepository,
    project_repo: ProjectRepository,
}

impl IngestReportUseCase {
    pub fn new(
        archive_repo: ArchiveRepository,
        queue_repo: QueueRepository,
        project_repo: ProjectRepository,
    ) -> Self {
        Self {
            archive_repo,
            queue_repo,
            project_repo,
        }
    }

    pub fn execute(
        &self,
        project_id: i32,
        hash: String,
        compressed_payload: Vec<u8>,
        original_size: Option<i32>,
    ) -> Result<String, DomainError> {
        if !self.project_repo.exists(project_id)? {
            return Err(DomainError::ProjectNotFound(project_id));
        }

        let archive_exists = self.archive_repo.exists(&hash)?;

        if !archive_exists {
            let archive = Archive::new(hash.clone(), compressed_payload, original_size);
            self.archive_repo.save(&archive)?;

            let queue_item = ProcessingQueueItem::new(hash.clone());
            self.queue_repo.enqueue(&queue_item)?;
        }

        Ok(hash)
    }
}
