use std::env;

pub struct Settings {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub worker_interval_secs: u64,
    pub worker_batch_size: usize,
    pub max_concurrent_compressions: usize,
    // Rate limiting (requests per second, 0 = disabled)
    pub rate_limit_global_per_sec: u64,
    pub rate_limit_per_ip_per_sec: u64,
    pub rate_limit_per_project_per_sec: u64,
    pub rate_limit_burst_multiplier: u32,
    // Analytics
    pub analytics_flush_interval_secs: u64,
    pub analytics_retention_days: i64,
    pub analytics_buffer_size: usize,
    // Database connection pool
    pub db_pool_size: u32,
    pub db_pool_timeout_secs: u64,
    // Request payload limits
    pub max_compressed_payload_bytes: usize,
    pub max_uncompressed_payload_bytes: usize,
}

impl Settings {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            database_url: Self::require_env("DATABASE_URL"),
            server_host: Self::require_env("SERVER_HOST"),
            server_port: Self::require_env_parse("SERVER_PORT"),

            // Worker settings
            worker_interval_secs: Self::require_env_parse("WORKER_INTERVAL_SECS"),
            worker_batch_size: Self::require_env_parse_or_fallback(
                "WORKER_REPORTS_BATCH_SIZE",
                "DIGEST_BATCH_SIZE",
            ),

            // Concurrency
            max_concurrent_compressions: Self::require_env_parse("MAX_CONCURRENT_COMPRESSIONS"),

            // Rate limiting
            rate_limit_global_per_sec: Self::require_env_parse_or_fallback(
                "RATE_LIMIT_REQUESTS_PER_SEC",
                "RATE_LIMIT_GLOBAL_PER_SEC",
            ),
            rate_limit_per_ip_per_sec: Self::require_env_parse("RATE_LIMIT_PER_IP_PER_SEC"),
            rate_limit_per_project_per_sec: Self::require_env_parse(
                "RATE_LIMIT_PER_PROJECT_PER_SEC",
            ),
            rate_limit_burst_multiplier: Self::require_env_parse("RATE_LIMIT_BURST_MULTIPLIER"),

            // Analytics
            analytics_flush_interval_secs: Self::require_env_parse("ANALYTICS_FLUSH_INTERVAL_SECS"),
            analytics_retention_days: Self::require_env_parse("ANALYTICS_RETENTION_DAYS"),
            analytics_buffer_size: Self::require_env_parse_or_fallback(
                "ANALYTICS_BUFFER_SIZE",
                "ANALYTICS_CHANNEL_BUFFER_SIZE",
            ),

            // Database pool
            db_pool_size: Self::require_env_parse_or_fallback(
                "DATABASE_POOL_SIZE",
                "DB_POOL_MAX_SIZE",
            ),
            db_pool_timeout_secs: Self::require_env_parse_or_fallback(
                "DATABASE_POOL_TIMEOUT_SECS",
                "DB_POOL_CONNECTION_TIMEOUT_SECS",
            ),

            // Payload limits
            max_compressed_payload_bytes: Self::require_env_parse("MAX_COMPRESSED_PAYLOAD_BYTES"),
            max_uncompressed_payload_bytes: Self::require_env_parse(
                "MAX_UNCOMPRESSED_PAYLOAD_BYTES",
            ),
        }
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }

    /// Calculate worker budget (90% of interval to prevent overlap)
    pub fn worker_budget_secs(&self) -> u64 {
        (self.worker_interval_secs as f64 * 0.9) as u64
    }

    // Helper functions
    fn require_env(key: &str) -> String {
        env::var(key).unwrap_or_else(|_| panic!("Missing required environment variable: {}", key))
    }

    fn require_env_parse<T>(key: &str) -> T
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        let value = Self::require_env(key);
        Self::parse_value(&value, key)
    }

    fn require_env_parse_or_fallback<T>(new_key: &str, old_key: &str) -> T
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        let value = env::var(new_key)
            .or_else(|_| {
                let val = env::var(old_key);
                if val.is_ok() {
                    eprintln!(
                        "Warning: '{}' is deprecated, use '{}' instead",
                        old_key, new_key
                    );
                }
                val
            })
            .unwrap_or_else(|_| {
                panic!(
                    "Missing required environment variable: {} (or deprecated: {})",
                    new_key, old_key
                )
            });

        Self::parse_value(&value, new_key)
    }

    fn parse_value<T>(value: &str, key: &str) -> T
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        let value = value.trim();

        // Try direct parse first
        if let Ok(parsed) = value.parse::<T>() {
            return parsed;
        }

        // For numeric types, try parsing multiplication expressions
        if value.contains('*') {
            if let Some(result) = Self::parse_multiplication(value) {
                // This is a bit tricky - we need to convert usize to T
                // This works for usize, u64, u32, etc.
                return format!("{}", result)
                    .parse()
                    .unwrap_or_else(|e| panic!("Failed to parse '{}' for {}: {}", value, key, e));
            }
        }

        panic!("Failed to parse '{}' for {}: invalid format", value, key)
    }

    /// Parse multiplication expression (e.g., "100 * 1024" or "5 * 1024 * 1024")
    fn parse_multiplication(value: &str) -> Option<usize> {
        let parts: Vec<&str> = value.split('*').map(|s| s.trim()).collect();

        match parts.len() {
            2 => {
                let left = parts[0].parse::<usize>().ok()?;
                let right = parts[1].parse::<usize>().ok()?;
                Some(left * right)
            }
            3 => {
                let first = parts[0].parse::<usize>().ok()?;
                let second = parts[1].parse::<usize>().ok()?;
                let third = parts[2].parse::<usize>().ok()?;
                Some(first * second * third)
            }
            _ => None,
        }
    }
}
