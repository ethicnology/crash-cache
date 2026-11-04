use diesel::Connection;
use sha2::{Digest, Sha256};
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

        // Content addressing is scoped to a project: two DSNs must never share ownership.
        let mut scoped_hash = Sha256::new();
        scoped_hash.update(project_id.to_be_bytes());
        scoped_hash.update(hash.as_bytes());
        let hash = hex::encode(scoped_hash.finalize());
        conn.transaction(|conn| {
            let archive = Archive::new(hash.clone(), project_id, compressed_payload, original_size);
            let inserted = self.archive_repo.save(conn, &archive)?;
            if inserted {
                self.queue_repo.enqueue(conn, &QueueItem::new(hash.clone()))?;
            }
            Ok(IngestResult { hash, duplicate: !inserted })
        })
    }
}
