//! FingerPain Core Library
//!
//! Provides database storage, metrics aggregation, and export functionality
//! for the FingerPain typing analytics tracker.

pub mod db;
pub mod export;
pub mod metrics;
pub mod session;

pub use db::Database;
pub use export::{ExportFormat, Exporter};
pub use metrics::{Metrics, TimeRange};
pub use session::SessionTracker;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single keystroke record (aggregated per minute per app)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystrokeRecord {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub app_name: Option<String>,
    pub app_bundle_id: Option<String>,
    pub char_count: u32,
    pub word_count: u32,
    pub paragraph_count: u32,
    pub backspace_count: u32,
    pub browser_domain: Option<String>,
    pub browser_url: Option<String>,
}

impl KeystrokeRecord {
    pub fn new(timestamp: DateTime<Utc>) -> Self {
        Self {
            id: None,
            timestamp,
            app_name: None,
            app_bundle_id: None,
            char_count: 0,
            word_count: 0,
            paragraph_count: 0,
            backspace_count: 0,
            browser_domain: None,
            browser_url: None,
        }
    }

    pub fn with_app(mut self, name: Option<String>, bundle_id: Option<String>) -> Self {
        self.app_name = name;
        self.app_bundle_id = bundle_id;
        self
    }
}

/// A typing session for WPM calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingSession {
    pub id: Option<i64>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub char_count: u32,
    pub word_count: u32,
    pub wpm_avg: Option<f64>,
    pub wpm_peak: Option<f64>,
}

impl TypingSession {
    pub fn new(start_time: DateTime<Utc>) -> Self {
        Self {
            id: None,
            start_time,
            end_time: None,
            char_count: 0,
            word_count: 0,
            wpm_avg: None,
            wpm_peak: None,
        }
    }
}

/// Aggregated stats for a time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedStats {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_chars: u64,
    pub total_words: u64,
    pub total_paragraphs: u64,
    pub total_backspaces: u64,
    pub net_chars: i64,
    pub avg_wpm: Option<f64>,
    pub peak_wpm: Option<f64>,
    pub active_minutes: u32,
}

/// Per-app statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStats {
    pub app_name: String,
    pub app_bundle_id: String,
    pub total_chars: u64,
    pub total_words: u64,
    pub percentage: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_domains: Option<Vec<DomainStats>>,
}

/// Domain statistics within a browser
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainStats {
    pub domain: String,
    pub total_chars: u64,
    pub total_words: u64,
    pub percentage: f64,
}

/// Browser context from active tab
#[derive(Debug, Clone)]
pub struct BrowserContext {
    pub domain: String,
    pub url: String,
    pub title: String,
}

/// Hourly breakdown for heatmap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyStats {
    pub hour: u8,
    pub day_of_week: u8,
    pub avg_chars: f64,
    pub avg_words: f64,
}

/// Peak typing time info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeakInfo {
    pub timestamp: DateTime<Utc>,
    pub char_count: u64,
    pub word_count: u64,
    pub duration_minutes: u32,
}

/// Get the data directory for FingerPain
pub fn data_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("com", "fingerpain", "fingerpain")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            directories::BaseDirs::new()
                .map(|d| d.home_dir().join(".fingerpain"))
                .unwrap_or_else(|| std::path::PathBuf::from(".fingerpain"))
        })
}

/// Get the database file path
pub fn db_path() -> std::path::PathBuf {
    data_dir().join("fingerpain.db")
}
