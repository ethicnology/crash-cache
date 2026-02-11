use crate::shared::domain::{Archive, DomainError, QueueItem};
use crate::shared::persistence::{
    ArchiveRepository, DbConnection, ProjectRepository, QueueRepository,
};

pub struct IngestResult {
    pub hash: String,
    pub duplicate: bool,
}

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
        conn: &mut DbConnection,
        project_id: i32,
        hash: String,
        compressed_payload: Vec<u8>,
        original_size: Option<i32>,
    ) -> Result<IngestResult, DomainError> {
        if !self.project_repo.exists(conn, project_id)? {
            return Err(DomainError::ProjectNotFound(project_id));
        }

        let archive_exists = self.archive_repo.exists(conn, &hash)?;

        if !archive_exists {
            let archive = Archive::new(hash.clone(), project_id, compressed_payload, original_size);
            self.archive_repo.save(conn, &archive)?;

            let queue_item = QueueItem::new(hash.clone());
            self.queue_repo.enqueue(conn, &queue_item)?;
        }

        Ok(IngestResult {
            hash,
            duplicate: archive_exists,
        })
    }
}
