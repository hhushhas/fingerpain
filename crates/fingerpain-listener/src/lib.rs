//! Cross-platform keystroke listener for FingerPain
//!
//! Uses the `rdev` crate for capturing keyboard events across macOS, Windows, and Linux.

pub mod counter;
pub mod platform;

use chrono::{DateTime, Utc};
use fingerpain_core::KeystrokeRecord;
use rdev::{Event, EventType, Key};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;

pub use counter::KeystrokeCounter;
pub use platform::ActiveApp;

#[derive(Error, Debug)]
pub enum ListenerError {
    #[error("Failed to start listener: {0}")]
    StartFailed(String),
    #[error("Listener already running")]
    AlreadyRunning,
    #[error("Platform error: {0}")]
    Platform(String),
}

pub type Result<T> = std::result::Result<T, ListenerError>;

/// A keyboard event with metadata
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: KeyEventType,
    pub app: Option<ActiveApp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    Character,
    Space,
    Enter,
    Backspace,
    Tab,
    Other,
}

impl KeyEventType {
    fn from_key(key: Key) -> Self {
        match key {
            Key::Space => KeyEventType::Space,
            Key::Return => KeyEventType::Enter,
            Key::Backspace => KeyEventType::Backspace,
            Key::Tab => KeyEventType::Tab,
            // Letter keys
            Key::KeyA | Key::KeyB | Key::KeyC | Key::KeyD | Key::KeyE |
            Key::KeyF | Key::KeyG | Key::KeyH | Key::KeyI | Key::KeyJ |
            Key::KeyK | Key::KeyL | Key::KeyM | Key::KeyN | Key::KeyO |
            Key::KeyP | Key::KeyQ | Key::KeyR | Key::KeyS | Key::KeyT |
            Key::KeyU | Key::KeyV | Key::KeyW | Key::KeyX | Key::KeyY |
            Key::KeyZ => KeyEventType::Character,
            // Number keys
            Key::Num0 | Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 |
            Key::Num5 | Key::Num6 | Key::Num7 | Key::Num8 | Key::Num9 => KeyEventType::Character,
            // Punctuation and symbols
            Key::Comma | Key::Dot | Key::Slash | Key::SemiColon |
            Key::Quote | Key::LeftBracket | Key::RightBracket |
            Key::BackSlash | Key::Minus | Key::Equal | Key::BackQuote => KeyEventType::Character,
            // Everything else
            _ => KeyEventType::Other,
        }
    }

    pub fn is_word_boundary(&self) -> bool {
        matches!(self, KeyEventType::Space | KeyEventType::Enter | KeyEventType::Tab)
    }
}

/// Callback type for keystroke events
pub type KeystrokeCallback = Box<dyn Fn(KeyEvent) + Send + 'static>;

/// The main keystroke listener
pub struct Listener {
    running: Arc<Mutex<bool>>,
    stop_tx: Option<Sender<()>>,
}

impl Listener {
    pub fn new() -> Self {
        Self {
            running: Arc::new(Mutex::new(false)),
            stop_tx: None,
        }
    }

    /// Check if listener is running
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    /// Start listening with a callback
    pub fn start<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(KeyEvent) + Send + 'static,
    {
        if self.is_running() {
            return Err(ListenerError::AlreadyRunning);
        }

        let running = self.running.clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        self.stop_tx = Some(stop_tx);

        *running.lock().unwrap() = true;

        // Start the listener in a separate thread
        thread::spawn(move || {
            Self::run_listener(callback, running, stop_rx);
        });

        Ok(())
    }

    fn run_listener<F>(callback: F, running: Arc<Mutex<bool>>, _stop_rx: Receiver<()>)
    where
        F: Fn(KeyEvent) + Send + 'static,
    {
        let callback = Arc::new(callback);
        let running_clone = running.clone();

        let result = rdev::listen(move |event: Event| {
            // Check if we should stop
            if !*running_clone.lock().unwrap() {
                return;
            }

            if let EventType::KeyPress(key) = event.event_type {
                let event_type = KeyEventType::from_key(key);

                // Only process actual typing keys, not modifiers
                if event_type != KeyEventType::Other {
                    let app = platform::get_active_app().ok();

                    let key_event = KeyEvent {
                        timestamp: Utc::now(),
                        event_type,
                        app,
                    };

                    callback(key_event);
                }
            }
        });

        if let Err(e) = result {
            tracing::error!("Listener error: {:?}", e);
        }

        *running.lock().unwrap() = false;
    }

    /// Stop listening
    pub fn stop(&mut self) {
        *self.running.lock().unwrap() = false;
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Default for Listener {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Aggregates keystrokes into records for database storage
pub struct KeystrokeAggregator {
    current_minute: i64,
    records: std::collections::HashMap<String, KeystrokeRecord>,
    counter: KeystrokeCounter,
}

impl KeystrokeAggregator {
    pub fn new() -> Self {
        Self {
            current_minute: 0,
            records: std::collections::HashMap::new(),
            counter: KeystrokeCounter::new(),
        }
    }

    /// Process a key event and return any completed records
    pub fn process(&mut self, event: KeyEvent) -> Vec<KeystrokeRecord> {
        let minute = event.timestamp.timestamp() / 60;
        let mut completed = Vec::new();

        // Check if we've moved to a new minute
        if minute != self.current_minute && self.current_minute != 0 {
            // Return all records from the previous minute
            completed = self.records.drain().map(|(_, r)| r).collect();
            self.counter.reset();
        }
        self.current_minute = minute;

        // Update counter
        self.counter.process(event.event_type);

        // Get or create record for this app
        let app_id = event
            .app
            .as_ref()
            .map(|a| a.bundle_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let record = self.records.entry(app_id.clone()).or_insert_with(|| {
            let mut r = KeystrokeRecord::new(event.timestamp);
            if let Some(app) = &event.app {
                r.app_name = Some(app.name.clone());
                r.app_bundle_id = Some(app.bundle_id.clone());
            }
            r
        });

        // Update counts
        match event.event_type {
            KeyEventType::Character | KeyEventType::Space | KeyEventType::Tab => {
                record.char_count += 1;
            }
            KeyEventType::Enter => {
                record.char_count += 1;
                record.paragraph_count += 1;
            }
            KeyEventType::Backspace => {
                record.backspace_count += 1;
            }
            KeyEventType::Other => {}
        }

        // Check for word completion
        if event.event_type.is_word_boundary() && self.counter.pending_chars > 0 {
            record.word_count += 1;
        }

        completed
    }

    /// Flush all pending records
    pub fn flush(&mut self) -> Vec<KeystrokeRecord> {
        self.counter.reset();
        self.records.drain().map(|(_, r)| r).collect()
    }
}

impl Default for KeystrokeAggregator {
    fn default() -> Self {
        Self::new()
    }
}
