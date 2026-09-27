use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use std::net::SocketAddr;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tower::{Layer, Service};
use tower_governor::{
    GovernorError, GovernorLayer,
    governor::GovernorConfigBuilder,
    key_extractor::{GlobalKeyExtractor, KeyExtractor, SmartIpKeyExtractor},
};

use tracing::warn;

use crate::shared::analytics::AnalyticsCollector;

fn mask_ip(ip: &str) -> String {
    if let Ok(addr) = ip.parse::<std::net::IpAddr>() {
        match addr {
            std::net::IpAddr::V4(v4) => {
                let o = v4.octets();
                format!("{}.{}.{}.x", o[0], o[1], o[2])
            }
            std::net::IpAddr::V6(v6) => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}::x", s[0], s[1], s[2])
            }
        }
    } else {
        "unknown".to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectKeyExtractor;

impl KeyExtractor for ProjectKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let path = req.uri().path();
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 3 && parts[1] == "api" {
            return Ok(parts[2].to_string());
        }
        Ok("_global".to_string())
    }
}

#[derive(Clone)]
pub struct AnalyticsLayer {
    collector: AnalyticsCollector,
}

impl AnalyticsLayer {
    pub fn new(collector: AnalyticsCollector) -> Self {
        Self { collector }
    }
}

impl<S> Layer<S> for AnalyticsLayer {
    type Service = AnalyticsMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AnalyticsMiddleware {
            inner,
            collector: self.collector.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AnalyticsMiddleware<S> {
    inner: S,
    collector: AnalyticsCollector,
}

impl<S> Service<Request<Body>> for AnalyticsMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let start = Instant::now();
        let endpoint = req.uri().path().to_string();
        let collector = self.collector.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let response = inner.call(req).await?;
            let latency_ms = start.elapsed().as_millis() as u32;
            collector.record_request_latency(endpoint, latency_ms);
            Ok(response)
        })
    }
}

#[derive(Clone)]
pub struct RateLimitAnalyticsLayer {
    collector: AnalyticsCollector,
    limit_type: RateLimitType,
}

#[derive(Clone, Copy)]
pub enum RateLimitType {
    Global,
    Ip,
    Project,
}

impl RateLimitAnalyticsLayer {
    pub fn new(collector: AnalyticsCollector, limit_type: RateLimitType) -> Self {
        Self {
            collector,
            limit_type,
        }
    }
}

impl<S> Layer<S> for RateLimitAnalyticsLayer {
    type Service = RateLimitAnalyticsMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitAnalyticsMiddleware {
            inner,
            collector: self.collector.clone(),
            limit_type: self.limit_type,
        }
    }
}

#[derive(Clone)]
pub struct RateLimitAnalyticsMiddleware<S> {
    inner: S,
    collector: AnalyticsCollector,
    limit_type: RateLimitType,
}

impl<S> Service<Request<Body>> for RateLimitAnalyticsMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let collector = self.collector.clone();
        let limit_type = self.limit_type;
        let mut inner = self.inner.clone();

        let ip = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|s| s.trim().to_string())
            .or_else(|| {
                req.headers()
                    .get("x-real-ip")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.trim().to_string())
            })
            .or_else(|| {
                req.extensions()
                    .get::<axum::extract::ConnectInfo<SocketAddr>>()
                    .map(|ci| ci.0.ip().to_string())
            });
        let dsn = {
            let path = req.uri().path();
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() >= 3 && parts[1] == "api" {
                Some(parts[2].to_string())
            } else {
                None
            }
        };

        Box::pin(async move {
            let response = inner.call(req).await?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                match limit_type {
                    RateLimitType::Global => {
                        warn!(subnet = ip.as_deref().map(|i| mask_ip(i)).as_deref().unwrap_or("unknown"), "Rate limit GLOBAL");
                        collector.record_rate_limit_global();
                    }
                    RateLimitType::Ip => {
                        if let Some(ip) = ip {
                            warn!(subnet = %mask_ip(&ip), "Rate limit IP");
                            collector.record_rate_limit_subnet(ip);
                        }
                    }
                    RateLimitType::Project => {
                        if let Some(dsn) = dsn {
                            warn!(project = %dsn, "Rate limit PROJECT");
                            collector.record_rate_limit_dsn(dsn, None);
                        }
                    }
                }
            }

            Ok(response)
        })
    }
}

/// Rate limit layers type alias to simplify return types
pub type IpRateLimitLayer = GovernorLayer<
    SmartIpKeyExtractor,
    governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>,
    axum::body::Body,
>;

pub type ProjectRateLimitLayer = GovernorLayer<
    ProjectKeyExtractor,
    governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>,
    axum::body::Body,
>;

pub type GlobalRateLimitLayer = GovernorLayer<
    GlobalKeyExtractor,
    governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>,
    axum::body::Body,
>;

// Shared by all three scopes so their requests-per-second contract stays identical.
fn rate_limit_settings(requests_per_sec: u64, burst_multiplier: u32) -> Option<(Duration, u32)> {
    if requests_per_sec == 0 || burst_multiplier == 0 {
        return None;
    }
    // tower_governor 0.8's per_second argument is seconds per token, not RPS.
    // Round up to nanosecond precision so the configured rate is never exceeded.
    let period = Duration::from_nanos(1_000_000_000_u64.div_ceil(requests_per_sec));
    let burst_size = requests_per_sec
        .saturating_mul(u64::from(burst_multiplier))
        .min(u64::from(u32::MAX)) as u32;
    Some((period, burst_size))
}

/// Creates a GovernorLayer for per-IP rate limiting using SmartIpKeyExtractor
pub fn create_ip_rate_limiter(
    requests_per_sec: u64,
    burst_multiplier: u32,
) -> Option<IpRateLimitLayer> {
    let (period, burst_size) = rate_limit_settings(requests_per_sec, burst_multiplier)?;
    let config = GovernorConfigBuilder::default()
        .period(period)
        .burst_size(burst_size)
        .key_extractor(SmartIpKeyExtractor)
        .finish()?;

    Some(GovernorLayer::new(config))
}

/// Creates a GovernorLayer for per-project rate limiting
pub fn create_project_rate_limiter(
    requests_per_sec: u64,
    burst_multiplier: u32,
) -> Option<ProjectRateLimitLayer> {
    let (period, burst_size) = rate_limit_settings(requests_per_sec, burst_multiplier)?;
    let config = GovernorConfigBuilder::default()
        .period(period)
        .burst_size(burst_size)
        .key_extractor(ProjectKeyExtractor)
        .finish()?;

    Some(GovernorLayer::new(config))
}

/// Creates a GovernorLayer for global rate limiting
pub fn create_global_rate_limiter(
    requests_per_sec: u64,
    burst_multiplier: u32,
) -> Option<GlobalRateLimitLayer> {
    let (period, burst_size) = rate_limit_settings(requests_per_sec, burst_multiplier)?;
    let config = GovernorConfigBuilder::default()
        .period(period)
        .burst_size(burst_size)
        .key_extractor(GlobalKeyExtractor)
        .finish()?;

    Some(GovernorLayer::new(config))
}

#[cfg(test)]
mod quota_tests {
    use super::*;

    #[test]
    fn configured_requests_per_second_replenishes_one_token_at_reciprocal_interval() {
        let (period, burst) = rate_limit_settings(500, 2).unwrap();
        assert_eq!(period, Duration::from_millis(2));
        assert_eq!(burst, 1000);
        assert_eq!(rate_limit_settings(1, 2), Some((Duration::from_secs(1), 2)));
        assert_eq!(rate_limit_settings(3, 1), Some((Duration::from_nanos(333_333_334), 3)));
    }

    #[test]
    fn large_quotas_saturate_burst_without_disabling_limiting() {
        assert_eq!(rate_limit_settings(u64::MAX, u32::MAX), Some((Duration::from_nanos(1), u32::MAX)));
        assert_eq!(rate_limit_settings(u32::MAX as u64 + 1, 1), Some((Duration::from_nanos(1), u32::MAX)));
        assert!(create_ip_rate_limiter(u64::MAX, u32::MAX).is_some());
        assert!(create_project_rate_limiter(u64::MAX, u32::MAX).is_some());
        assert!(create_global_rate_limiter(u64::MAX, u32::MAX).is_some());
    }

    #[test]
    fn zero_configuration_preserves_disabled_scope_behavior() {
        assert_eq!(rate_limit_settings(0, 2), None);
        assert_eq!(rate_limit_settings(10, 0), None);
        assert!(create_ip_rate_limiter(0, 2).is_none());
        assert!(create_project_rate_limiter(0, 2).is_none());
        assert!(create_global_rate_limiter(0, 2).is_none());
    }
}
