use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct ProcessingQueueItem {
    pub id: Option<i32>,
    pub event_id: i32,
    pub created_at: DateTime<Utc>,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<DateTime<Utc>>,
}

impl ProcessingQueueItem {
    pub fn new(event_id: i32) -> Self {
        Self {
            id: None,
            event_id,
            created_at: Utc::now(),
            retry_count: 0,
            last_error: None,
            next_retry_at: None,
        }
    }

    pub fn increment_retry(&mut self, error: String, backoff_seconds: i64) {
        self.retry_count += 1;
        self.last_error = Some(error);
        self.next_retry_at = Some(Utc::now() + chrono::Duration::seconds(backoff_seconds));
    }

    pub fn is_ready_for_retry(&self) -> bool {
        match self.next_retry_at {
            Some(next) => Utc::now() >= next,
            None => true,
        }
    }
}
