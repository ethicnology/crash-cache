use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{info, warn};

use super::DigestReportUseCase;

pub struct DigestWorker {
    digest_use_case: DigestReportUseCase,
    interval_secs: u64,
    processing_budget_secs: u64,
    batch_size: usize,
    shutdown: Arc<AtomicBool>,
}

impl DigestWorker {
    pub fn new(
        digest_use_case: DigestReportUseCase,
        interval_secs: u64,
        processing_budget_secs: u64,
        batch_size: usize,
    ) -> Self {
        Self {
            digest_use_case,
            interval_secs,
            processing_budget_secs,
            batch_size,
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

        loop {
            if start.elapsed() >= budget {
                if total_processed > 0 {
                    info!(
                        total_processed = total_processed,
                        elapsed_ms = start.elapsed().as_millis(),
                        "Processing budget exhausted"
                    );
                }
                break;
            }

            if self.shutdown.load(Ordering::SeqCst) {
                break;
            }

            match self.digest_use_case.process_batch(self.batch_size as i32) {
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

        if total_processed > 0 {
            info!(
                total_processed = total_processed,
                elapsed_ms = start.elapsed().as_millis(),
                "Processing tick completed"
            );
        }
    }
}
