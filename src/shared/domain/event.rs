use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Event {
    pub id: Option<i32>,
    pub project_id: String,
    pub archive_hash: String,
    pub received_at: DateTime<Utc>,
    pub processed: bool,
}

impl Event {
    pub fn new(project_id: String, archive_hash: String) -> Self {
        Self {
            id: None,
            project_id,
            archive_hash,
            received_at: Utc::now(),
            processed: false,
        }
    }

    pub fn with_id(mut self, id: i32) -> Self {
        self.id = Some(id);
        self
    }
}
