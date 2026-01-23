use std::env;

pub struct Settings {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub worker_interval_secs: u64,
    pub worker_budget_secs: u64,
    pub max_concurrent_compressions: usize,
    pub health_cache_ttl_secs: u64,
    // Rate limiting (requests per second, 0 = disabled)
    pub rate_limit_global_per_sec: u64,
    pub rate_limit_per_ip_per_sec: u64,
    pub rate_limit_per_project_per_sec: u64,
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
            max_concurrent_compressions: env::var("MAX_CONCURRENT_COMPRESSIONS")
                .unwrap_or_else(|_| "16".to_string())
                .parse()
                .expect("MAX_CONCURRENT_COMPRESSIONS must be a valid number"),
            health_cache_ttl_secs: env::var("HEALTH_CACHE_TTL_SECS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("HEALTH_CACHE_TTL_SECS must be a valid number"),
            rate_limit_global_per_sec: env::var("RATE_LIMIT_GLOBAL_PER_SEC")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .expect("RATE_LIMIT_GLOBAL_PER_SEC must be a valid number"),
            rate_limit_per_ip_per_sec: env::var("RATE_LIMIT_PER_IP_PER_SEC")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("RATE_LIMIT_PER_IP_PER_SEC must be a valid number"),
            rate_limit_per_project_per_sec: env::var("RATE_LIMIT_PER_PROJECT_PER_SEC")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .expect("RATE_LIMIT_PER_PROJECT_PER_SEC must be a valid number"),
        }
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
