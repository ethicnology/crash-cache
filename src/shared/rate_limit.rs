use axum::http::Request;
use tower_governor::{
    governor::GovernorConfigBuilder,
    key_extractor::{GlobalKeyExtractor, KeyExtractor, SmartIpKeyExtractor},
    GovernorError, GovernorLayer,
};

/// Extracts project_id from URL path (/api/{project_id}/...)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectKeyExtractor;

impl KeyExtractor for ProjectKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let path = req.uri().path();

        // Parse /api/{project_id}/store or /api/{project_id}/envelope
        let parts: Vec<&str> = path.split('/').collect();

        // Expected: ["", "api", "project_id", "store|envelope", ...]
        if parts.len() >= 3 && parts[1] == "api" {
            return Ok(parts[2].to_string());
        }

        // Fallback for non-API routes (like /health)
        Ok("_global".to_string())
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
