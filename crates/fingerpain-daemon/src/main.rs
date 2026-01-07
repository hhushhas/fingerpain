//! FingerPain Daemon
//!
//! Background service that listens to keystrokes and records them.
//! On macOS, rdev::listen requires running on the main thread with CFRunLoop.

use anyhow::Result;
use fingerpain_core::db::Database;
use fingerpain_core::KeystrokeRecord;
use chrono::Utc;
use rdev::{listen, Event, EventType, Key};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tracing::{error, info};

/// Tracks keystrokes per minute per app
struct KeystrokeTracker {
    db: Database,
    current_minute: i64,
    records: HashMap<String, KeystrokeRecord>,
    pending_word_chars: u32,
}

impl KeystrokeTracker {
    fn new(db: Database) -> Self {
        Self {
            db,
            current_minute: 0,
            records: HashMap::new(),
            pending_word_chars: 0,
        }
    }

    fn process_key(&mut self, key: Key) {
        let now = Utc::now();
        let minute = now.timestamp() / 60;

        // If we moved to a new minute, flush old records
        if minute != self.current_minute && self.current_minute != 0 {
            self.flush();
        }
        self.current_minute = minute;

        // Determine key type
        let (is_char, is_word_boundary, is_backspace, is_enter) = match key {
            Key::Space => (true, true, false, false),
            Key::Tab => (true, true, false, false),
            Key::Return => (true, true, false, true),
            Key::Backspace => (false, false, true, false),
            Key::KeyA | Key::KeyB | Key::KeyC | Key::KeyD | Key::KeyE |
            Key::KeyF | Key::KeyG | Key::KeyH | Key::KeyI | Key::KeyJ |
            Key::KeyK | Key::KeyL | Key::KeyM | Key::KeyN | Key::KeyO |
            Key::KeyP | Key::KeyQ | Key::KeyR | Key::KeyS | Key::KeyT |
            Key::KeyU | Key::KeyV | Key::KeyW | Key::KeyX | Key::KeyY |
            Key::KeyZ => (true, false, false, false),
            Key::Num0 | Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 |
            Key::Num5 | Key::Num6 | Key::Num7 | Key::Num8 | Key::Num9 => (true, false, false, false),
            Key::Comma | Key::Dot | Key::Slash | Key::SemiColon |
            Key::Quote | Key::LeftBracket | Key::RightBracket |
            Key::BackSlash | Key::Minus | Key::Equal | Key::BackQuote => (true, false, false, false),
            _ => (false, false, false, false),
        };

        // Skip if not a relevant key
        if !is_char && !is_backspace {
            return;
        }

        // Get or create record for "unknown" app (simplified - no app detection for now)
        let app_id = "system".to_string();
        let record = self.records.entry(app_id.clone()).or_insert_with(|| {
            let mut r = KeystrokeRecord::new(now);
            r.app_name = Some("System".to_string());
            r.app_bundle_id = Some("system".to_string());
            r
        });

        // Update counts
        if is_char {
            record.char_count += 1;
            if !is_word_boundary {
                self.pending_word_chars += 1;
            }
        }

        if is_backspace {
            record.backspace_count += 1;
            if self.pending_word_chars > 0 {
                self.pending_word_chars -= 1;
            }
        }

        if is_enter {
            record.paragraph_count += 1;
        }

        // Word completed on boundary if we had pending chars
        if is_word_boundary && self.pending_word_chars > 0 {
            record.word_count += 1;
            self.pending_word_chars = 0;
        }
    }

    fn flush(&mut self) {
        for (_, record) in self.records.drain() {
            if record.char_count > 0 || record.backspace_count > 0 {
                if let Err(e) = self.db.upsert_keystroke(&record) {
                    error!("Failed to save keystroke: {}", e);
                } else {
                    info!(
                        "Saved: {} chars, {} words, {} paragraphs",
                        record.char_count, record.word_count, record.paragraph_count
                    );
                }
            }
        }
    }
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fingerpain=info".parse()?),
        )
        .init();

    info!("FingerPain daemon starting...");

    // Open database
    let db = Database::open_default()?;
    info!("Database opened at {:?}", fingerpain_core::db_path());

    // Create tracker wrapped in Arc<Mutex> for callback
    let tracker = Arc::new(Mutex::new(KeystrokeTracker::new(db)));
    let tracker_clone = tracker.clone();

    info!("Starting keystroke listener (press Ctrl+C to stop)...");

    // This blocks and runs on main thread - required for macOS CGEventTap
    if let Err(e) = listen(move |event: Event| {
        if let EventType::KeyPress(key) = event.event_type {
            if let Ok(mut t) = tracker_clone.lock() {
                t.process_key(key);
            }
        }
    }) {
        error!("Listener error: {:?}", e);
        return Err(anyhow::anyhow!("Failed to start listener: {:?}", e));
    }

    // Flush remaining data on exit
    if let Ok(mut t) = tracker.lock() {
        t.flush();
    }

    info!("FingerPain daemon stopped");
    Ok(())
}
