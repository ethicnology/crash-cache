use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crash_cache::config::Settings;
use crash_cache::features::process_report::{ProcessCrashUseCase, ProcessingWorker};
use crash_cache::features::receive_report::{create_router, AppState, IngestCrashUseCase};
use crash_cache::shared::compression::GzipCompressor;
use crash_cache::shared::persistence::{
    establish_connection_pool, run_migrations, ArchiveRepository, CrashMetadataRepository,
    EventRepository, QueueRepository,
};

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    let settings = Settings::from_env();
    info!("Starting crash-cache server");

    let pool = establish_connection_pool(&settings.database_url);
    run_migrations(&pool);
    info!("Database initialized");

    let archive_repo = ArchiveRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let queue_repo = QueueRepository::new(pool.clone());
    let metadata_repo = CrashMetadataRepository::new(pool);
    let compressor = GzipCompressor::new();

    let ingest_use_case = IngestCrashUseCase::new(
        archive_repo.clone(),
        event_repo.clone(),
        queue_repo.clone(),
        compressor.clone(),
    );

    let process_use_case = ProcessCrashUseCase::new(
        archive_repo,
        event_repo,
        queue_repo,
        metadata_repo,
        compressor,
    );

    let worker = ProcessingWorker::new(
        process_use_case,
        settings.worker_interval_secs,
        settings.worker_budget_secs,
    );
    let shutdown_handle = worker.shutdown_handle();

    let worker_handle = tokio::spawn(async move {
        worker.run().await;
    });

    let app_state = AppState { ingest_use_case };
    let app = create_router(app_state);

    let addr = settings.server_addr();
    info!(addr = %addr, "Server listening");

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
