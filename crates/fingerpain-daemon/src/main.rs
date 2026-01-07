//! FingerPain Daemon
//!
//! Background service that listens to keystrokes and records them.
//! On macOS, rdev::listen requires running on the main thread with CFRunLoop.

use anyhow::Result;
use fingerpain_core::db::Database;
use fingerpain_core::KeystrokeRecord;
use fingerpain_listener::platform;
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
    last_app_check: i64,
    cached_app: Option<platform::ActiveApp>,
}

impl KeystrokeTracker {
    fn new(db: Database) -> Self {
        Self {
            db,
            current_minute: 0,
            records: HashMap::new(),
            pending_word_chars: 0,
            last_app_check: 0,
            cached_app: None,
        }
    }

    fn is_browser(bundle_id: &str) -> bool {
        matches!(
            bundle_id,
            "com.JadeApps.Helium"
                | "com.google.Chrome"
                | "org.mozilla.firefox"
                | "com.apple.Safari"
        )
    }

    fn process_key(&mut self, key: Key) {
        let now = Utc::now();
        let minute = now.timestamp() / 60;
        let current_time = now.timestamp();

        // If we moved to a new minute, flush old records
        if minute != self.current_minute && self.current_minute != 0 {
            self.flush();
        }
        self.current_minute = minute;

        // Check active app every 2 seconds (cache it)
        if current_time - self.last_app_check >= 2 {
            self.cached_app = platform::get_active_app().ok();
            self.last_app_check = current_time;
        }

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

        // Get app info and browser context
        let (app_name, bundle_id, browser_domain, browser_url): (Option<String>, Option<String>, Option<String>, Option<String>) =
            if let Some(ref app) = self.cached_app {
                let mut domain: Option<String> = None;
                let mut url: Option<String> = None;

                // If browser, try to fetch latest domain context
                if Self::is_browser(&app.bundle_id) {
                    if let Ok(Some(ctx)) = self.db.get_browser_context(&app.bundle_id) {
                        domain = Some(ctx.domain);
                        url = Some(ctx.url);
                    }
                }

                (
                    Some(app.name.clone()),
                    Some(app.bundle_id.clone()),
                    domain,
                    url,
                )
            } else {
                (None, None, None, None)
            };

        // Create record key from bundle ID (or "unknown" if no app detected)
        let app_id: String = bundle_id
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Get or create record
        let record = self.records.entry(app_id.clone()).or_insert_with(|| {
            let mut r = KeystrokeRecord::new(now);
            r.app_name = app_name.clone();
            r.app_bundle_id = bundle_id.clone();
            r.browser_domain = browser_domain.clone();
            r.browser_url = browser_url.clone();
            r
        });

        // Update browser context if this is a browser (latest info)
        if let (Some(bd), Some(bu)) = (&browser_domain, &browser_url) {
            record.browser_domain = Some(bd.clone());
            record.browser_url = Some(bu.clone());
        }

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
                    let app_info = record
                        .app_name
                        .as_ref()
                        .map(|n| n.as_str())
                        .unwrap_or("Unknown");
                    let browser_info = record
                        .browser_domain
                        .as_ref()
                        .map(|d| format!(" → {}", d))
                        .unwrap_or_default();
                    info!(
                        "Saved: {} {}{} | {} chars, {} words, {} paragraphs",
                        app_info,
                        browser_info,
                        "",
                        record.char_count,
                        record.word_count,
                        record.paragraph_count
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
