use std::env;

pub struct Settings {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub worker_interval_secs: u64,
    pub worker_budget_secs: u64,
}

impl Settings {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "crash_cache.db".to_string()),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("SERVER_PORT must be a valid port number"),
            worker_interval_secs: env::var("WORKER_INTERVAL_SECS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .expect("WORKER_INTERVAL_SECS must be a valid number"),
            worker_budget_secs: env::var("WORKER_BUDGET_SECS")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .expect("WORKER_BUDGET_SECS must be a valid number"),
        }
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
