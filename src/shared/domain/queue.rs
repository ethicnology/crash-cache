use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: Option<i32>,
    pub archive_hash: String,
    pub created_at: DateTime<Utc>,
}

impl QueueItem {
    pub fn new(archive_hash: String) -> Self {
        Self {
            id: None,
            archive_hash,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueError {
    pub id: i32,
    pub archive_hash: String,
    pub error: String,
    pub created_at: DateTime<Utc>,
}
