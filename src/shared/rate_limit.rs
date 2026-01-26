use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use std::net::SocketAddr;
use std::task::{Context, Poll};
use std::time::Instant;
use tower::{Layer, Service};
use tower_governor::{
    governor::GovernorConfigBuilder,
    key_extractor::{GlobalKeyExtractor, KeyExtractor, SmartIpKeyExtractor},
    GovernorError, GovernorLayer,
};

use crate::shared::analytics::AnalyticsCollector;

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
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

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
        Self { collector, limit_type }
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
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let collector = self.collector.clone();
        let limit_type = self.limit_type;
        let mut inner = self.inner.clone();

        let ip = req.extensions()
            .get::<axum::extract::ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip().to_string());
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
                        collector.record_rate_limit_global();
                    }
                    RateLimitType::Ip => {
                        if let Some(ip) = ip {
                            collector.record_rate_limit_subnet(ip);
                        }
                    }
                    RateLimitType::Project => {
                        if let Some(dsn) = dsn {
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

/// Creates a GovernorLayer for per-IP rate limiting using SmartIpKeyExtractor
pub fn create_ip_rate_limiter(requests_per_sec: u64) -> Option<IpRateLimitLayer> {
    if requests_per_sec == 0 {
        return None;
    }

    let config = GovernorConfigBuilder::default()
        .per_second(requests_per_sec)
        .burst_size(requests_per_sec as u32 * 2)
        .key_extractor(SmartIpKeyExtractor)
        .finish()?;

    Some(GovernorLayer::new(config))
}

/// Creates a GovernorLayer for per-project rate limiting
pub fn create_project_rate_limiter(requests_per_sec: u64) -> Option<ProjectRateLimitLayer> {
    if requests_per_sec == 0 {
        return None;
    }

    let config = GovernorConfigBuilder::default()
        .per_second(requests_per_sec)
        .burst_size(requests_per_sec as u32 * 2)
        .key_extractor(ProjectKeyExtractor)
        .finish()?;

    Some(GovernorLayer::new(config))
}

/// Creates a GovernorLayer for global rate limiting
pub fn create_global_rate_limiter(requests_per_sec: u64) -> Option<GlobalRateLimitLayer> {
    if requests_per_sec == 0 {
        return None;
    }

    let config = GovernorConfigBuilder::default()
        .per_second(requests_per_sec)
        .burst_size(requests_per_sec as u32 * 2)
        .key_extractor(GlobalKeyExtractor)
        .finish()?;

    Some(GovernorLayer::new(config))
}
