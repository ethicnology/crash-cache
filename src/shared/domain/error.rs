use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Decompression error: {0}")]
    Decompression(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Processing error: {0}")]
    Processing(String),

    #[error("Max retries exceeded for archive {0}")]
    MaxRetriesExceeded(String),

    #[error("Project not found: {0}")]
    ProjectNotFound(i32),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Duplicate event_id: {0}")]
    DuplicateEventId(String),
}
