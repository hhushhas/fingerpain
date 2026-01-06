//! Export functionality for CSV and JSON formats

use crate::{db::Database, AggregatedStats, AppStats, KeystrokeRecord, TimeRange};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::io::Write;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("Database error: {0}")]
    Db(#[from] crate::db::DbError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ExportError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
}

impl ExportFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "csv" => Some(ExportFormat::Csv),
            "json" => Some(ExportFormat::Json),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        }
    }
}

/// Export data structure for JSON
#[derive(Debug, Serialize)]
pub struct ExportData {
    pub exported_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub summary: AggregatedStats,
    pub app_breakdown: Vec<AppStats>,
    pub records: Vec<KeystrokeRecord>,
}

pub struct Exporter<'a> {
    db: &'a Database,
}

impl<'a> Exporter<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Export data for a time range to a writer
    pub fn export<W: Write>(
        &self,
        writer: W,
        range: TimeRange,
        format: ExportFormat,
    ) -> Result<()> {
        let (start, end) = range.to_range();

        match format {
            ExportFormat::Csv => self.export_csv(writer, start, end),
            ExportFormat::Json => self.export_json(writer, start, end),
        }
    }

    fn export_csv<W: Write>(
        &self,
        writer: W,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<()> {
        let records = self.db.get_all_records(start, end)?;

        let mut csv_writer = csv::Writer::from_writer(writer);

        // Write header
        csv_writer.write_record([
            "timestamp",
            "app_name",
            "app_bundle_id",
            "char_count",
            "word_count",
            "paragraph_count",
            "backspace_count",
        ])?;

        // Write records
        for record in records {
            csv_writer.write_record([
                record.timestamp.to_rfc3339(),
                record.app_name.unwrap_or_default(),
                record.app_bundle_id.unwrap_or_default(),
                record.char_count.to_string(),
                record.word_count.to_string(),
                record.paragraph_count.to_string(),
                record.backspace_count.to_string(),
            ])?;
        }

        csv_writer.flush()?;
        Ok(())
    }

    fn export_json<W: Write>(
        &self,
        mut writer: W,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<()> {
        let summary = self.db.get_stats(start, end)?;
        let app_breakdown = self.db.get_app_stats(start, end)?;
        let records = self.db.get_all_records(start, end)?;

        let export_data = ExportData {
            exported_at: Utc::now(),
            period_start: start,
            period_end: end,
            summary,
            app_breakdown,
            records,
        };

        let json = serde_json::to_string_pretty(&export_data)?;
        writer.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Export summary only (no raw records)
    pub fn export_summary<W: Write>(
        &self,
        mut writer: W,
        range: TimeRange,
        format: ExportFormat,
    ) -> Result<()> {
        let (start, end) = range.to_range();
        let summary = self.db.get_stats(start, end)?;
        let app_breakdown = self.db.get_app_stats(start, end)?;

        match format {
            ExportFormat::Csv => {
                let mut csv_writer = csv::Writer::from_writer(writer);
                csv_writer.write_record([
                    "metric",
                    "value",
                ])?;
                csv_writer.write_record(["total_chars", &summary.total_chars.to_string()])?;
                csv_writer.write_record(["total_words", &summary.total_words.to_string()])?;
                csv_writer.write_record(["total_paragraphs", &summary.total_paragraphs.to_string()])?;
                csv_writer.write_record(["total_backspaces", &summary.total_backspaces.to_string()])?;
                csv_writer.write_record(["net_chars", &summary.net_chars.to_string()])?;
                csv_writer.write_record(["active_minutes", &summary.active_minutes.to_string()])?;
                if let Some(wpm) = summary.avg_wpm {
                    csv_writer.write_record(["avg_wpm", &format!("{:.1}", wpm)])?;
                }
                if let Some(wpm) = summary.peak_wpm {
                    csv_writer.write_record(["peak_wpm", &format!("{:.1}", wpm)])?;
                }
                csv_writer.flush()?;
            }
            ExportFormat::Json => {
                #[derive(Serialize)]
                struct SummaryExport {
                    exported_at: DateTime<Utc>,
                    period_start: DateTime<Utc>,
                    period_end: DateTime<Utc>,
                    summary: AggregatedStats,
                    app_breakdown: Vec<AppStats>,
                }

                let export = SummaryExport {
                    exported_at: Utc::now(),
                    period_start: start,
                    period_end: end,
                    summary,
                    app_breakdown,
                };

                let json = serde_json::to_string_pretty(&export)?;
                writer.write_all(json.as_bytes())?;
            }
        }

        Ok(())
    }
}
