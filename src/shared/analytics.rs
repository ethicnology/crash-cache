use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender};
use tracing::{debug, error, info, warn};

use crate::shared::persistence::AnalyticsRepository;

const DEFAULT_FLUSH_INTERVAL_SECS: u64 = 10;
const DEFAULT_RETENTION_DAYS: i64 = 30;

#[derive(Debug, Clone)]
pub enum AnalyticsEvent {
    RateLimitGlobal,
    RateLimitDsn {
        dsn: String,
        project_id: Option<i32>,
    },
    RateLimitSubnet {
        ip: String,
    },
    RequestLatency {
        endpoint: String,
        latency_ms: u32,
    },
}

#[derive(Default)]
struct EventBuffer {
    global_hits: i64,
    dsn_hits: HashMap<(String, Option<i32>), i64>,
    subnet_hits: HashMap<String, i64>,
    latency: HashMap<String, LatencyStats>,
}

struct LatencyStats {
    count: i64,
    total_ms: i64,
    min_ms: i32,
    max_ms: i32,
}

#[derive(Clone)]
pub struct AnalyticsCollector {
    sender: Sender<AnalyticsEvent>,
}

impl AnalyticsCollector {
    pub fn new(
        repo: AnalyticsRepository,
        flush_interval_secs: Option<u64>,
        retention_days: Option<i64>,
        channel_buffer_size: usize,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(channel_buffer_size);
        let flush_interval = flush_interval_secs.unwrap_or(DEFAULT_FLUSH_INTERVAL_SECS);
        let retention = retention_days.unwrap_or(DEFAULT_RETENTION_DAYS);

        tokio::spawn(async move {
            Self::run_collector(receiver, repo, flush_interval, retention).await;
        });

        Self { sender }
    }

    pub fn record(&self, event: AnalyticsEvent) {
        if let Err(e) = self.sender.try_send(event) {
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    debug!("Analytics channel full, dropping event");
                }
                mpsc::error::TrySendError::Closed(_) => {
                    warn!("Analytics channel closed");
                }
            }
        }
    }

    pub fn record_rate_limit_global(&self) {
        self.record(AnalyticsEvent::RateLimitGlobal);
    }

    pub fn record_rate_limit_dsn(&self, dsn: String, project_id: Option<i32>) {
        self.record(AnalyticsEvent::RateLimitDsn { dsn, project_id });
    }

    pub fn record_rate_limit_subnet(&self, ip: String) {
        self.record(AnalyticsEvent::RateLimitSubnet { ip });
    }

    pub fn record_request_latency(&self, endpoint: String, latency_ms: u32) {
        self.record(AnalyticsEvent::RequestLatency {
            endpoint,
            latency_ms,
        });
    }

    async fn run_collector(
        mut receiver: mpsc::Receiver<AnalyticsEvent>,
        repo: AnalyticsRepository,
        flush_interval_secs: u64,
        retention_days: i64,
    ) {
        let mut buffer = EventBuffer::default();
        let mut flush_interval = tokio::time::interval(Duration::from_secs(flush_interval_secs));
        let mut cleanup_interval = tokio::time::interval(Duration::from_secs(3600));

        info!(
            flush_interval_secs = flush_interval_secs,
            retention_days = retention_days,
            "Analytics collector started"
        );

        loop {
            tokio::select! {
                Some(event) = receiver.recv() => {
                    Self::buffer_event(&mut buffer, event);
                }
                _ = flush_interval.tick() => {
                    Self::flush_buffer(&mut buffer, &repo);
                }
                _ = cleanup_interval.tick() => {
                    Self::cleanup_old_data(&repo, retention_days);
                }
            }
        }
    }

    fn buffer_event(buffer: &mut EventBuffer, event: AnalyticsEvent) {
        match event {
            AnalyticsEvent::RateLimitGlobal => {
                buffer.global_hits += 1;
            }
            AnalyticsEvent::RateLimitDsn { dsn, project_id } => {
                *buffer.dsn_hits.entry((dsn, project_id)).or_insert(0) += 1;
            }
            AnalyticsEvent::RateLimitSubnet { ip } => {
                let subnet = Self::ip_to_subnet(&ip);
                *buffer.subnet_hits.entry(subnet).or_insert(0) += 1;
            }
            AnalyticsEvent::RequestLatency {
                endpoint,
                latency_ms,
            } => {
                let latency = latency_ms as i32;
                buffer
                    .latency
                    .entry(endpoint)
                    .and_modify(|stats| {
                        stats.count += 1;
                        stats.total_ms += latency as i64;
                        stats.min_ms = stats.min_ms.min(latency);
                        stats.max_ms = stats.max_ms.max(latency);
                    })
                    .or_insert(LatencyStats {
                        count: 1,
                        total_ms: latency as i64,
                        min_ms: latency,
                        max_ms: latency,
                    });
            }
        }
    }

    fn flush_buffer(buffer: &mut EventBuffer, repo: &AnalyticsRepository) {
        let total_events = buffer.global_hits
            + buffer.dsn_hits.values().sum::<i64>()
            + buffer.subnet_hits.values().sum::<i64>()
            + buffer.latency.values().map(|s| s.count).sum::<i64>();

        if total_events == 0 {
            return;
        }

        debug!(events = total_events, "Flushing analytics buffer");

        for _ in 0..buffer.global_hits {
            if let Err(e) = repo.record_rate_limit_global() {
                error!(error = %e, "Failed to record global rate limit");
            }
        }

        for ((dsn, project_id), count) in buffer.dsn_hits.drain() {
            for _ in 0..count {
                if let Err(e) = repo.record_rate_limit_dsn(&dsn, project_id) {
                    error!(error = %e, dsn = %dsn, "Failed to record DSN rate limit");
                }
            }
        }

        for (subnet, count) in buffer.subnet_hits.drain() {
            for _ in 0..count {
                if let Err(e) = repo.record_rate_limit_subnet(&subnet) {
                    error!(error = %e, subnet = %subnet, "Failed to record subnet rate limit");
                }
            }
        }

        for (endpoint, stats) in buffer.latency.drain() {
            for _ in 0..stats.count {
                let avg_latency = (stats.total_ms / stats.count) as u32;
                if let Err(e) = repo.record_request_latency(&endpoint, avg_latency) {
                    error!(error = %e, endpoint = %endpoint, "Failed to record request latency");
                }
            }
        }

        buffer.global_hits = 0;
    }

    fn cleanup_old_data(repo: &AnalyticsRepository, retention_days: i64) {
        match repo.cleanup_old_buckets(retention_days) {
            Ok(deleted) if deleted > 0 => {
                info!(deleted = deleted, "Cleaned up old analytics buckets");
            }
            Ok(_) => {}
            Err(e) => {
                error!(error = %e, "Failed to cleanup old analytics data");
            }
        }
    }

    fn ip_to_subnet(ip: &str) -> String {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() >= 3 {
            format!("{}.{}.{}", parts[0], parts[1], parts[2])
        } else if ip.contains(':') {
            let parts: Vec<&str> = ip.split(':').collect();
            if parts.len() >= 4 {
                format!("{}:{}:{}:{}", parts[0], parts[1], parts[2], parts[3])
            } else {
                ip.to_string()
            }
        } else {
            ip.to_string()
        }
    }
}
