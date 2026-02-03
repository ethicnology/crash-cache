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
    // Analytics
    pub analytics_flush_interval_secs: u64,
    pub analytics_retention_days: i64,
    // Database connection pool
    pub db_pool_max_size: u32,
    pub db_pool_connection_timeout_secs: u64,
    // Request payload limits
    pub max_compressed_payload_bytes: usize,
    pub max_uncompressed_payload_bytes: usize,
    // Digest worker
    pub digest_batch_size: usize,
    // Rate limiting burst multipliers
    pub rate_limit_burst_multiplier: u32,
    // Analytics channel buffer
    pub analytics_channel_buffer_size: usize,
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
            analytics_flush_interval_secs: env::var("ANALYTICS_FLUSH_INTERVAL_SECS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("ANALYTICS_FLUSH_INTERVAL_SECS must be a valid number"),
            analytics_retention_days: env::var("ANALYTICS_RETENTION_DAYS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .expect("ANALYTICS_RETENTION_DAYS must be a valid number"),
            db_pool_max_size: env::var("DB_POOL_MAX_SIZE")
                .expect("DB_POOL_MAX_SIZE must be set")
                .parse()
                .expect("DB_POOL_MAX_SIZE must be a valid number"),
            db_pool_connection_timeout_secs: env::var("DB_POOL_CONNECTION_TIMEOUT_SECS")
                .expect("DB_POOL_CONNECTION_TIMEOUT_SECS must be set")
                .parse()
                .expect("DB_POOL_CONNECTION_TIMEOUT_SECS must be a valid number"),
            max_compressed_payload_bytes: env::var("MAX_COMPRESSED_PAYLOAD_BYTES")
                .expect("MAX_COMPRESSED_PAYLOAD_BYTES must be set")
                .parse()
                .expect("MAX_COMPRESSED_PAYLOAD_BYTES must be a valid number"),
            max_uncompressed_payload_bytes: env::var("MAX_UNCOMPRESSED_PAYLOAD_BYTES")
                .expect("MAX_UNCOMPRESSED_PAYLOAD_BYTES must be set")
                .parse()
                .expect("MAX_UNCOMPRESSED_PAYLOAD_BYTES must be a valid number"),
            digest_batch_size: env::var("DIGEST_BATCH_SIZE")
                .expect("DIGEST_BATCH_SIZE must be set")
                .parse()
                .expect("DIGEST_BATCH_SIZE must be a valid number"),
            rate_limit_burst_multiplier: env::var("RATE_LIMIT_BURST_MULTIPLIER")
                .expect("RATE_LIMIT_BURST_MULTIPLIER must be set")
                .parse()
                .expect("RATE_LIMIT_BURST_MULTIPLIER must be a valid number"),
            analytics_channel_buffer_size: env::var("ANALYTICS_CHANNEL_BUFFER_SIZE")
                .expect("ANALYTICS_CHANNEL_BUFFER_SIZE must be set")
                .parse()
                .expect("ANALYTICS_CHANNEL_BUFFER_SIZE must be a valid number"),
        }
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
