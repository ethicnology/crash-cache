use axum::extract::DefaultBodyLimit;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::Semaphore;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crash_cache::config::Settings;
use crash_cache::features::digest::{DigestReportUseCase, DigestWorker};
use crash_cache::features::ingest::{create_router, AppState, IngestReportUseCase};
use crash_cache::shared::compression::GzipCompressor;
use crash_cache::shared::persistence::{establish_connection_pool, run_migrations, Repositories};

const MAX_BODY_SIZE: usize = 1 * 1024 * 1024;

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    let settings = Settings::from_env();
    info!("Starting crash-cache server");

    let pool = establish_connection_pool(&settings.database_url);
    run_migrations(&pool);
    info!("Database initialized");

    let repos = Repositories::new(pool);
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestReportUseCase::new(
        repos.archive.clone(),
        repos.queue.clone(),
        repos.project.clone(),
    );

    let digest_use_case = DigestReportUseCase::new(repos.clone(), compressor, 1);

    let worker = DigestWorker::new(
        digest_use_case,
        settings.worker_interval_secs,
        settings.worker_budget_secs,
    );
    let shutdown_handle = worker.shutdown_handle();

    let worker_handle = tokio::spawn(async move {
        worker.run().await;
    });

    let compression_semaphore = Arc::new(Semaphore::new(settings.max_concurrent_compressions));
    info!(
        max_concurrent_compressions = settings.max_concurrent_compressions,
        "Compression semaphore initialized"
    );

    let app_state = AppState {
        ingest_use_case,
        compression_semaphore,
    };
    let app = create_router(app_state).layer(DefaultBodyLimit::max(MAX_BODY_SIZE));

    let addr = settings.server_addr();
    info!(addr = %addr, "Server listening");
    info!("DSN format: http://<key>@{addr}/<project_id>");

    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

    axum::serve(listener, app)
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
