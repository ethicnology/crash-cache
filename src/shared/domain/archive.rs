use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Archive {
    pub hash: String,
    pub compressed_payload: Vec<u8>,
    pub original_size: Option<i32>,
    pub created_at: DateTime<Utc>,
}

impl Archive {
    pub fn new(hash: String, compressed_payload: Vec<u8>, original_size: Option<i32>) -> Self {
        Self {
            hash,
            compressed_payload,
            original_size,
            created_at: Utc::now(),
        }
    }
}
