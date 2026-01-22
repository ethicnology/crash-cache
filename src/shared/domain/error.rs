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

    #[error("Max retries exceeded for event {0}")]
    MaxRetriesExceeded(i32),

    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}
