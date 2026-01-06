//! Typing session tracking for WPM calculation

use crate::{db::Database, TypingSession};
use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};

/// Tracks typing sessions and calculates WPM
pub struct SessionTracker {
    db: Arc<Database>,
    current_session: Mutex<Option<ActiveSession>>,
    /// Idle timeout before ending a session (default: 5 seconds)
    idle_timeout: Duration,
}

struct ActiveSession {
    session: TypingSession,
    last_keystroke: DateTime<Utc>,
    /// Rolling window of (timestamp, char_count) for WPM calculation
    keystroke_times: Vec<(DateTime<Utc>, u32)>,
    current_wpm: f64,
    peak_wpm: f64,
}

impl SessionTracker {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            current_session: Mutex::new(None),
            idle_timeout: Duration::seconds(5),
        }
    }

    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Record a keystroke event
    pub fn record_keystroke(&self, char_count: u32, word_count: u32) -> crate::db::Result<()> {
        let now = Utc::now();
        let mut session_guard = self.current_session.lock().unwrap();

        match session_guard.as_mut() {
            Some(active) => {
                // Check if we should end the current session due to idle
                if now - active.last_keystroke > self.idle_timeout {
                    // End current session
                    let mut session = active.session.clone();
                    session.end_time = Some(active.last_keystroke);
                    session.wpm_avg = Some(active.calculate_avg_wpm());
                    session.wpm_peak = Some(active.peak_wpm);

                    if session.id.is_some() {
                        self.db.update_session(&session)?;
                    }

                    // Start new session
                    let new_session = self.start_new_session(now, char_count, word_count)?;
                    *session_guard = Some(new_session);
                } else {
                    // Update current session
                    active.session.char_count += char_count;
                    active.session.word_count += word_count;
                    active.last_keystroke = now;

                    // Add to rolling window
                    active.keystroke_times.push((now, char_count));

                    // Remove old entries (older than 60 seconds)
                    let cutoff = now - Duration::seconds(60);
                    active.keystroke_times.retain(|(t, _)| *t > cutoff);

                    // Calculate current WPM
                    active.current_wpm = active.calculate_current_wpm();
                    if active.current_wpm > active.peak_wpm {
                        active.peak_wpm = active.current_wpm;
                    }
                }
            }
            None => {
                // Start new session
                let new_session = self.start_new_session(now, char_count, word_count)?;
                *session_guard = Some(new_session);
            }
        }

        Ok(())
    }

    fn start_new_session(
        &self,
        now: DateTime<Utc>,
        char_count: u32,
        word_count: u32,
    ) -> crate::db::Result<ActiveSession> {
        let mut session = TypingSession::new(now);
        session.char_count = char_count;
        session.word_count = word_count;

        let id = self.db.insert_session(&session)?;
        session.id = Some(id);

        Ok(ActiveSession {
            session,
            last_keystroke: now,
            keystroke_times: vec![(now, char_count)],
            current_wpm: 0.0,
            peak_wpm: 0.0,
        })
    }

    /// Get current WPM (0 if no active session)
    pub fn current_wpm(&self) -> f64 {
        self.current_session
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.current_wpm)
            .unwrap_or(0.0)
    }

    /// Get peak WPM for current session
    pub fn peak_wpm(&self) -> f64 {
        self.current_session
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.peak_wpm)
            .unwrap_or(0.0)
    }

    /// Check for idle and end session if needed
    pub fn check_idle(&self) -> crate::db::Result<()> {
        let now = Utc::now();
        let mut session_guard = self.current_session.lock().unwrap();

        if let Some(active) = session_guard.as_mut() {
            if now - active.last_keystroke > self.idle_timeout {
                // End session
                let mut session = active.session.clone();
                session.end_time = Some(active.last_keystroke);
                session.wpm_avg = Some(active.calculate_avg_wpm());
                session.wpm_peak = Some(active.peak_wpm);

                if session.id.is_some() {
                    self.db.update_session(&session)?;
                }

                *session_guard = None;
            }
        }

        Ok(())
    }

    /// Force end the current session
    pub fn end_session(&self) -> crate::db::Result<()> {
        let now = Utc::now();
        let mut session_guard = self.current_session.lock().unwrap();

        if let Some(active) = session_guard.take() {
            let wpm_avg = active.calculate_avg_wpm();
            let wpm_peak = active.peak_wpm;
            let mut session = active.session;
            session.end_time = Some(now);
            session.wpm_avg = Some(wpm_avg);
            session.wpm_peak = Some(wpm_peak);

            if session.id.is_some() {
                self.db.update_session(&session)?;
            }
        }

        Ok(())
    }
}

impl ActiveSession {
    /// Calculate current WPM based on recent keystrokes (last 60 seconds)
    fn calculate_current_wpm(&self) -> f64 {
        if self.keystroke_times.len() < 2 {
            return 0.0;
        }

        let total_chars: u32 = self.keystroke_times.iter().map(|(_, c)| c).sum();
        let first_time = self.keystroke_times.first().unwrap().0;
        let last_time = self.keystroke_times.last().unwrap().0;

        let duration_secs = (last_time - first_time).num_seconds() as f64;
        if duration_secs <= 0.0 {
            return 0.0;
        }

        // Assume average word is 5 characters
        let words = total_chars as f64 / 5.0;
        let minutes = duration_secs / 60.0;

        words / minutes
    }

    /// Calculate average WPM for the entire session
    fn calculate_avg_wpm(&self) -> f64 {
        let duration = self.last_keystroke - self.session.start_time;
        let duration_secs = duration.num_seconds() as f64;

        if duration_secs <= 0.0 {
            return 0.0;
        }

        // Assume average word is 5 characters
        let words = self.session.char_count as f64 / 5.0;
        let minutes = duration_secs / 60.0;

        words / minutes
    }
}
