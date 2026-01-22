use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{info, warn};

use super::ProcessReportUseCase;

pub struct ProcessingWorker {
    process_use_case: ProcessReportUseCase,
    interval_secs: u64,
    processing_budget_secs: u64,
    shutdown: Arc<AtomicBool>,
}

impl ProcessingWorker {
    pub fn new(
        process_use_case: ProcessReportUseCase,
        interval_secs: u64,
        processing_budget_secs: u64,
    ) -> Self {
        Self {
            process_use_case,
            interval_secs,
            processing_budget_secs,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    pub async fn run(&self) {
        info!(
            interval_secs = self.interval_secs,
            budget_secs = self.processing_budget_secs,
            "Starting processing worker"
        );

        let mut ticker = interval(Duration::from_secs(self.interval_secs));

        loop {
            ticker.tick().await;

            if self.shutdown.load(Ordering::SeqCst) {
                info!("Processing worker shutting down");
                break;
            }

            self.process_tick();
        }
    }

    fn process_tick(&self) {
        let start = Instant::now();
        let budget = Duration::from_secs(self.processing_budget_secs);
        let mut total_processed = 0u32;
        let batch_size = 50;

        info!("Processing tick started");

        loop {
            if start.elapsed() >= budget {
                info!(
                    total_processed = total_processed,
                    elapsed_ms = start.elapsed().as_millis(),
                    "Processing budget exhausted"
                );
                break;
            }

            if self.shutdown.load(Ordering::SeqCst) {
                break;
            }

            match self.process_use_case.process_batch(batch_size) {
                Ok(processed) => {
                    total_processed += processed;
                    if processed == 0 {
                        break;
                    }
                    info!(processed = processed, "Batch processed");
                }
                Err(e) => {
                    warn!(error = %e, "Error processing batch (continuing)");
                }
            }
        }

        info!(
            total_processed = total_processed,
            elapsed_ms = start.elapsed().as_millis(),
            "Processing tick completed"
        );
    }
}
