//! FingerPain Daemon
//!
//! Background service that listens to keystrokes and records them.

use anyhow::Result;
use fingerpain_core::db::Database;
use fingerpain_listener::{KeyEvent, KeystrokeAggregator, Listener};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;
use tracing::{error, info};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fingerpain=info".parse()?),
        )
        .init();

    info!("FingerPain daemon starting...");

    // Open database (stays in main thread)
    let db = Database::open_default()?;
    info!("Database opened at {:?}", fingerpain_core::db_path());

    // Create channel for keystroke events
    let (tx, rx) = mpsc::channel::<KeyEvent>();

    // Create aggregator
    let aggregator = Arc::new(Mutex::new(KeystrokeAggregator::new()));

    // Setup graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    ctrlc::set_handler(move || {
        info!("Shutdown signal received");
        running_clone.store(false, Ordering::SeqCst);
    })?;

    // Start listener
    let mut listener = Listener::new();

    listener.start(move |event| {
        // Send event to main thread via channel
        if let Err(e) = tx.send(event) {
            error!("Failed to send event: {}", e);
        }
    })?;

    info!("Keystroke listener started");
    info!("Press Ctrl+C to stop");

    // Main loop - process events and save to database
    while running.load(Ordering::SeqCst) {
        // Process any pending events (non-blocking)
        while let Ok(event) = rx.try_recv() {
            // Process through aggregator
            let completed_records = {
                let mut agg = aggregator.lock().unwrap();
                agg.process(event)
            };

            // Save completed records to database
            for record in completed_records {
                if let Err(e) = db.upsert_keystroke(&record) {
                    error!("Failed to save keystroke record: {}", e);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    // Graceful shutdown
    info!("Shutting down...");
    listener.stop();

    // Flush any remaining data
    {
        let mut agg = aggregator.lock().unwrap();
        let remaining = agg.flush();
        for record in remaining {
            if let Err(e) = db.upsert_keystroke(&record) {
                error!("Failed to save final record: {}", e);
            }
        }
    }

    info!("FingerPain daemon stopped");
    Ok(())
}
