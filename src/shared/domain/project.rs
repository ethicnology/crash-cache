use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub public_key: Option<String>,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Project {
    pub fn new(id: String) -> Self {
        Self {
            id,
            public_key: None,
            name: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_public_key(mut self, key: Option<String>) -> Self {
        self.public_key = key;
        self
    }

    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = name;
        self
    }
}
