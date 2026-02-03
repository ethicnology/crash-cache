use axum::extract::DefaultBodyLimit;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::Semaphore;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use crate::config::Settings;
use crate::features::digest::{DigestReportUseCase, DigestWorker};
use crate::features::ingest::{
    AppState, HealthStats, IngestReportUseCase, create_api_router, create_health_router,
};
use crate::shared::analytics::AnalyticsCollector;
use crate::shared::compression::GzipCompressor;
use crate::shared::persistence::{Repositories, establish_connection_pool, run_migrations};
use crate::shared::rate_limit::{
    AnalyticsLayer, RateLimitAnalyticsLayer, RateLimitType, create_global_rate_limiter,
    create_ip_rate_limiter, create_project_rate_limiter,
};

pub async fn run_server() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    let settings = Settings::from_env();
    info!("Starting crash-cache server");

    let pool = establish_connection_pool(
        &settings.database_url,
        settings.db_pool_max_size,
        settings.db_pool_connection_timeout_secs,
    );
    run_migrations(&pool);
    info!("Database initialized");

    let repos = Repositories::new(pool.clone());
    let compressor = GzipCompressor::new();

    let analytics_collector = AnalyticsCollector::new(
        repos.analytics.clone(),
        Some(settings.analytics_flush_interval_secs),
        Some(settings.analytics_retention_days),
        settings.analytics_channel_buffer_size,
    );
    info!(
        flush_interval = settings.analytics_flush_interval_secs,
        retention_days = settings.analytics_retention_days,
        "Analytics collector initialized"
    );

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );

    let digest_use_case = DigestReportUseCase::new(repos.clone(), pool.clone(), compressor);

    let worker = DigestWorker::new(
        digest_use_case,
        settings.worker_interval_secs,
        settings.worker_budget_secs,
        settings.digest_batch_size,
    );
    let shutdown_handle = worker.shutdown_handle();

    let worker_handle = tokio::spawn(async move {
        worker.run().await;
    });

    // Spawn health stats refresh task
    let health_cache = Arc::new(RwLock::new(HealthStats::default()));
    let health_cache_for_task = health_cache.clone();
    let pool_for_health = pool.clone();
    let health_refresh_interval = Duration::from_secs(settings.worker_interval_secs);

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(health_refresh_interval).await;

            // Refresh stats in blocking task to avoid blocking Tokio threads
            let cache = health_cache_for_task.clone();
            let pool = pool_for_health.clone();

            tokio::task::spawn_blocking(move || {
                if let Ok(mut conn) = pool.get() {
                    let stats = crate::features::ingest::compute_health_stats(&mut conn);
                    if let Ok(mut cache_guard) = cache.write() {
                        *cache_guard = stats;
                    }
                }
            })
            .await
            .ok();
        }
    });

    info!(
        "Health stats refresh task started (interval: {}s)",
        settings.worker_interval_secs
    );

    let compression_semaphore = Arc::new(Semaphore::new(settings.max_concurrent_compressions));
    info!(
        max_concurrent_compressions = settings.max_concurrent_compressions,
        "Compression semaphore initialized"
    );

    // Use worker_interval_secs as cache TTL (reuse existing setting)
    let project_cache = crate::features::ingest::ProjectCache::new(Duration::from_secs(
        settings.worker_interval_secs,
    ));
    info!(
        project_cache_ttl_secs = settings.worker_interval_secs,
        "Project cache initialized"
    );

    let app_state = AppState {
        ingest_use_case,
        compression_semaphore,
        pool,
        project_repo: repos.project.clone(),
        project_cache,
        health_cache,
        health_cache_ttl: Duration::from_secs(settings.health_cache_ttl_secs),
        max_uncompressed_payload_bytes: settings.max_uncompressed_payload_bytes,
        // Session repositories
        session_repo: repos.session.clone(),
        session_status_repo: repos.session_status.clone(),
        session_release_repo: repos.session_release.clone(),
        session_environment_repo: repos.session_environment.clone(),
    };

    info!(
        global = settings.rate_limit_global_per_sec,
        per_ip = settings.rate_limit_per_ip_per_sec,
        per_project = settings.rate_limit_per_project_per_sec,
        "Rate limiting configured (0 = disabled)"
    );

    let mut api_router = create_api_router(app_state.clone())
        .layer(DefaultBodyLimit::max(settings.max_compressed_payload_bytes))
        .layer(AnalyticsLayer::new(analytics_collector.clone()));

    if let Some(layer) = create_ip_rate_limiter(
        settings.rate_limit_per_ip_per_sec,
        settings.rate_limit_burst_multiplier,
    ) {
        api_router = api_router
            .layer(RateLimitAnalyticsLayer::new(
                analytics_collector.clone(),
                RateLimitType::Ip,
            ))
            .layer(layer);
        info!("Per-IP rate limiter enabled");
    }

    if let Some(layer) = create_project_rate_limiter(
        settings.rate_limit_per_project_per_sec,
        settings.rate_limit_burst_multiplier,
    ) {
        api_router = api_router
            .layer(RateLimitAnalyticsLayer::new(
                analytics_collector.clone(),
                RateLimitType::Project,
            ))
            .layer(layer);
        info!("Per-project rate limiter enabled");
    }

    if let Some(layer) = create_global_rate_limiter(
        settings.rate_limit_global_per_sec,
        settings.rate_limit_burst_multiplier,
    ) {
        api_router = api_router
            .layer(RateLimitAnalyticsLayer::new(
                analytics_collector.clone(),
                RateLimitType::Global,
            ))
            .layer(layer);
        info!("Global rate limiter enabled");
    }

    // Health router without rate limiting
    let health_router = create_health_router(app_state);

    // Merge routers
    let app = api_router.merge(health_router);

    let addr = settings.server_addr();
    info!(addr = %addr, "Server listening");
    info!("DSN format: http://<key>@{addr}/<project_id>");

    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

    // Use into_make_service_with_connect_info to enable SmartIpKeyExtractor to access peer IP
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(shutdown_handle))
    .await
    .expect("Server error");

    worker_handle.await.ok();
    info!("Server shutdown complete");
}

async fn shutdown_signal(shutdown_handle: Arc<std::sync::atomic::AtomicBool>) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received");
    shutdown_handle.store(true, Ordering::SeqCst);
}
