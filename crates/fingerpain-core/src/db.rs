//! Database layer for FingerPain
//!
//! Handles all SQLite operations including schema creation, inserts, and queries.

use crate::{AggregatedStats, AppStats, BrowserContext, DomainStats, HourlyStats, KeystrokeRecord, PeakInfo, TypingSession};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::Path;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Record not found")]
    NotFound,
}

pub type Result<T> = std::result::Result<T, DbError>;

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Open the default database
    pub fn open_default() -> Result<Self> {
        Self::open(crate::db_path())
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            -- Keystroke records (per minute per app)
            CREATE TABLE IF NOT EXISTS keystrokes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                app_name TEXT,
                app_bundle_id TEXT,
                char_count INTEGER DEFAULT 0,
                word_count INTEGER DEFAULT 0,
                paragraph_count INTEGER DEFAULT 0,
                backspace_count INTEGER DEFAULT 0,
                UNIQUE(timestamp, app_bundle_id)
            );

            CREATE INDEX IF NOT EXISTS idx_keystrokes_timestamp ON keystrokes(timestamp);
            CREATE INDEX IF NOT EXISTS idx_keystrokes_app ON keystrokes(app_bundle_id);

            -- Typing sessions (for WPM tracking)
            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                start_time INTEGER NOT NULL,
                end_time INTEGER,
                char_count INTEGER DEFAULT 0,
                word_count INTEGER DEFAULT 0,
                wpm_avg REAL,
                wpm_peak REAL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_start ON sessions(start_time);

            -- Daily aggregates cache (for fast queries)
            CREATE TABLE IF NOT EXISTS daily_stats (
                date INTEGER PRIMARY KEY,
                total_chars INTEGER DEFAULT 0,
                total_words INTEGER DEFAULT 0,
                total_paragraphs INTEGER DEFAULT 0,
                total_backspaces INTEGER DEFAULT 0,
                active_minutes INTEGER DEFAULT 0,
                avg_wpm REAL,
                peak_wpm REAL
            );
            "#,
        )?;

        // Run migrations
        self.migrate_v1_browser_tracking()?;

        Ok(())
    }

    /// Migrate to browser tracking (v1)
    fn migrate_v1_browser_tracking(&self) -> Result<()> {
        // Check if browser_context table exists
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='browser_context'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !table_exists {
            info!("Running migration: browser tracking v1");

            self.conn.execute_batch(
                r#"
                CREATE TABLE browser_context (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp INTEGER NOT NULL,
                    browser_name TEXT NOT NULL,
                    url TEXT,
                    domain TEXT,
                    page_title TEXT,
                    last_updated INTEGER NOT NULL,
                    UNIQUE(browser_name)
                );

                CREATE INDEX idx_browser_context_browser ON browser_context(browser_name);
                CREATE INDEX idx_browser_context_updated ON browser_context(last_updated);
                "#,
            )?;
        }

        // Add browser columns to keystrokes table if they don't exist
        if !self.column_exists("keystrokes", "browser_domain")? {
            self.conn
                .execute("ALTER TABLE keystrokes ADD COLUMN browser_domain TEXT", [])?;
            self.conn
                .execute("ALTER TABLE keystrokes ADD COLUMN browser_url TEXT", [])?;
        }

        Ok(())
    }

    /// Check if a column exists in a table
    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info({})", table))?;
        let rows = stmt.query_map([], |row| {
            let col_name: String = row.get(1)?;
            Ok(col_name)
        })?;

        for row in rows {
            if row? == column {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Insert or update a keystroke record for the current minute
    pub fn upsert_keystroke(&self, record: &KeystrokeRecord) -> Result<i64> {
        let timestamp = record.timestamp.timestamp();
        let minute_timestamp = (timestamp / 60) * 60; // Round to minute

        self.conn.execute(
            r#"
            INSERT INTO keystrokes (timestamp, app_name, app_bundle_id, char_count, word_count, paragraph_count, backspace_count, browser_domain, browser_url)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(timestamp, app_bundle_id) DO UPDATE SET
                char_count = char_count + excluded.char_count,
                word_count = word_count + excluded.word_count,
                paragraph_count = paragraph_count + excluded.paragraph_count,
                backspace_count = backspace_count + excluded.backspace_count,
                browser_domain = COALESCE(excluded.browser_domain, browser_domain),
                browser_url = COALESCE(excluded.browser_url, browser_url)
            "#,
            params![
                minute_timestamp,
                record.app_name,
                record.app_bundle_id,
                record.char_count,
                record.word_count,
                record.paragraph_count,
                record.backspace_count,
                record.browser_domain,
                record.browser_url,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Insert a new typing session
    pub fn insert_session(&self, session: &TypingSession) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO sessions (start_time, end_time, char_count, word_count, wpm_avg, wpm_peak)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                session.start_time.timestamp(),
                session.end_time.map(|t| t.timestamp()),
                session.char_count,
                session.word_count,
                session.wpm_avg,
                session.wpm_peak,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Update an existing session
    pub fn update_session(&self, session: &TypingSession) -> Result<()> {
        let id = session.id.ok_or(DbError::NotFound)?;

        self.conn.execute(
            r#"
            UPDATE sessions SET
                end_time = ?2,
                char_count = ?3,
                word_count = ?4,
                wpm_avg = ?5,
                wpm_peak = ?6
            WHERE id = ?1
            "#,
            params![
                id,
                session.end_time.map(|t| t.timestamp()),
                session.char_count,
                session.word_count,
                session.wpm_avg,
                session.wpm_peak,
            ],
        )?;

        Ok(())
    }

    /// Get aggregated stats for a time range
    pub fn get_stats(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<AggregatedStats> {
        let start_ts = start.timestamp();
        let end_ts = end.timestamp();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                COALESCE(SUM(char_count), 0) as total_chars,
                COALESCE(SUM(word_count), 0) as total_words,
                COALESCE(SUM(paragraph_count), 0) as total_paragraphs,
                COALESCE(SUM(backspace_count), 0) as total_backspaces,
                COUNT(DISTINCT timestamp) as active_minutes
            FROM keystrokes
            WHERE timestamp >= ?1 AND timestamp < ?2
            "#,
        )?;

        let (total_chars, total_words, total_paragraphs, total_backspaces, active_minutes): (
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = stmt.query_row(params![start_ts, end_ts], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?;

        // Get WPM stats from sessions
        let mut wpm_stmt = self.conn.prepare(
            r#"
            SELECT AVG(wpm_avg), MAX(wpm_peak)
            FROM sessions
            WHERE start_time >= ?1 AND start_time < ?2 AND wpm_avg IS NOT NULL
            "#,
        )?;

        let (avg_wpm, peak_wpm): (Option<f64>, Option<f64>) =
            wpm_stmt.query_row(params![start_ts, end_ts], |row| Ok((row.get(0)?, row.get(1)?)))?;

        Ok(AggregatedStats {
            period_start: start,
            period_end: end,
            total_chars: total_chars as u64,
            total_words: total_words as u64,
            total_paragraphs: total_paragraphs as u64,
            total_backspaces: total_backspaces as u64,
            net_chars: total_chars - total_backspaces,
            avg_wpm,
            peak_wpm,
            active_minutes: active_minutes as u32,
        })
    }

    /// Get per-app statistics for a time range
    pub fn get_app_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AppStats>> {
        let start_ts = start.timestamp();
        let end_ts = end.timestamp();

        // First get total chars for percentage calculation
        let total: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(char_count), 0) FROM keystrokes WHERE timestamp >= ?1 AND timestamp < ?2",
            params![start_ts, end_ts],
            |row| row.get(0),
        )?;

        if total == 0 {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                COALESCE(app_name, 'Unknown') as app_name,
                COALESCE(app_bundle_id, 'unknown') as app_bundle_id,
                SUM(char_count) as total_chars,
                SUM(word_count) as total_words
            FROM keystrokes
            WHERE timestamp >= ?1 AND timestamp < ?2
            GROUP BY app_bundle_id
            ORDER BY total_chars DESC
            "#,
        )?;

        let rows = stmt.query_map(params![start_ts, end_ts], |row| {
            let chars: i64 = row.get(2)?;
            Ok(AppStats {
                app_name: row.get(0)?,
                app_bundle_id: row.get(1)?,
                total_chars: chars as u64,
                total_words: row.get::<_, i64>(3)? as u64,
                percentage: (chars as f64 / total as f64) * 100.0,
                browser_domains: None,
            })
        })?;

        rows.collect::<SqliteResult<Vec<_>>>().map_err(DbError::from)
    }

    /// Get hourly breakdown for heatmap
    pub fn get_hourly_stats(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<HourlyStats>> {
        let start_ts = start.timestamp();
        let end_ts = end.timestamp();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                CAST(strftime('%H', timestamp, 'unixepoch', 'localtime') AS INTEGER) as hour,
                CAST(strftime('%w', timestamp, 'unixepoch', 'localtime') AS INTEGER) as dow,
                AVG(char_count) as avg_chars,
                AVG(word_count) as avg_words
            FROM keystrokes
            WHERE timestamp >= ?1 AND timestamp < ?2
            GROUP BY hour, dow
            ORDER BY dow, hour
            "#,
        )?;

        let rows = stmt.query_map(params![start_ts, end_ts], |row| {
            Ok(HourlyStats {
                hour: row.get::<_, i64>(0)? as u8,
                day_of_week: row.get::<_, i64>(1)? as u8,
                avg_chars: row.get(2)?,
                avg_words: row.get(3)?,
            })
        })?;

        rows.collect::<SqliteResult<Vec<_>>>().map_err(DbError::from)
    }

    /// Get peak typing times
    pub fn get_peak_times(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<PeakInfo>> {
        let start_ts = start.timestamp();
        let end_ts = end.timestamp();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                timestamp,
                SUM(char_count) as total_chars,
                SUM(word_count) as total_words
            FROM keystrokes
            WHERE timestamp >= ?1 AND timestamp < ?2
            GROUP BY timestamp
            ORDER BY total_chars DESC
            LIMIT ?3
            "#,
        )?;

        let rows = stmt.query_map(params![start_ts, end_ts, limit as i64], |row| {
            let ts: i64 = row.get(0)?;
            Ok(PeakInfo {
                timestamp: Utc.timestamp_opt(ts, 0).unwrap(),
                char_count: row.get::<_, i64>(1)? as u64,
                word_count: row.get::<_, i64>(2)? as u64,
                duration_minutes: 1,
            })
        })?;

        rows.collect::<SqliteResult<Vec<_>>>().map_err(DbError::from)
    }

    /// Get daily totals for charting
    pub fn get_daily_totals(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<(DateTime<Utc>, u64, u64)>> {
        let start_ts = start.timestamp();
        let end_ts = end.timestamp();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                date(timestamp, 'unixepoch', 'localtime') as day,
                SUM(char_count) as chars,
                SUM(word_count) as words
            FROM keystrokes
            WHERE timestamp >= ?1 AND timestamp < ?2
            GROUP BY day
            ORDER BY day
            "#,
        )?;

        let rows = stmt.query_map(params![start_ts, end_ts], |row| {
            let day_str: String = row.get(0)?;
            let chars: i64 = row.get(1)?;
            let words: i64 = row.get(2)?;

            // Parse the date string
            let date = chrono::NaiveDate::parse_from_str(&day_str, "%Y-%m-%d")
                .unwrap_or_else(|_| chrono::Utc::now().date_naive());
            let datetime = date.and_hms_opt(0, 0, 0).unwrap();

            Ok((
                DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc),
                chars as u64,
                words as u64,
            ))
        })?;

        rows.collect::<SqliteResult<Vec<_>>>().map_err(DbError::from)
    }

    /// Get all keystroke records for export
    pub fn get_all_records(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<KeystrokeRecord>> {
        let start_ts = start.timestamp();
        let end_ts = end.timestamp();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, timestamp, app_name, app_bundle_id, char_count, word_count, paragraph_count, backspace_count, browser_domain, browser_url
            FROM keystrokes
            WHERE timestamp >= ?1 AND timestamp < ?2
            ORDER BY timestamp
            "#,
        )?;

        let rows = stmt.query_map(params![start_ts, end_ts], |row| {
            let ts: i64 = row.get(1)?;
            Ok(KeystrokeRecord {
                id: Some(row.get(0)?),
                timestamp: Utc.timestamp_opt(ts, 0).unwrap(),
                app_name: row.get(2)?,
                app_bundle_id: row.get(3)?,
                char_count: row.get::<_, i64>(4)? as u32,
                word_count: row.get::<_, i64>(5)? as u32,
                paragraph_count: row.get::<_, i64>(6)? as u32,
                backspace_count: row.get::<_, i64>(7)? as u32,
                browser_domain: row.get(8)?,
                browser_url: row.get(9)?,
            })
        })?;

        rows.collect::<SqliteResult<Vec<_>>>().map_err(DbError::from)
    }

    /// Upsert browser context
    pub fn upsert_browser_context(
        &self,
        browser_name: &str,
        url: &str,
        domain: &str,
        title: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO browser_context (browser_name, url, domain, page_title, timestamp, last_updated)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(browser_name) DO UPDATE SET
                url = excluded.url,
                domain = excluded.domain,
                page_title = excluded.page_title,
                last_updated = excluded.last_updated
            "#,
            params![browser_name, url, domain, title, now, now],
        )?;

        Ok(())
    }

    /// Get browser context for a specific browser bundle ID
    pub fn get_browser_context(&self, bundle_id: &str) -> Result<Option<BrowserContext>> {
        // Map bundle ID to browser name
        let browser_name = match bundle_id {
            "com.JadeApps.Helium" => "Helium",
            "com.google.Chrome" => "Chrome",
            "org.mozilla.firefox" => "Firefox",
            "com.apple.Safari" => "Safari",
            _ => return Ok(None),
        };

        let mut stmt = self.conn.prepare(
            r#"
            SELECT domain, url, page_title, last_updated
            FROM browser_context
            WHERE browser_name = ?1
            LIMIT 1
            "#,
        )?;

        let result = stmt.query_row(params![browser_name], |row| {
            Ok(BrowserContext {
                domain: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
            })
        });

        match result {
            Ok(ctx) => Ok(Some(ctx)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::from(e)),
        }
    }

    /// Get domain statistics for a browser within a time range
    pub fn get_browser_domains(
        &self,
        bundle_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        browser_total: u64,
    ) -> Result<Vec<DomainStats>> {
        let start_ts = start.timestamp();
        let end_ts = end.timestamp();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                COALESCE(browser_domain, 'Other') as domain,
                SUM(char_count) as total_chars,
                SUM(word_count) as total_words
            FROM keystrokes
            WHERE app_bundle_id = ?1
                AND timestamp >= ?2
                AND timestamp < ?3
            GROUP BY browser_domain
            ORDER BY total_chars DESC
            LIMIT 20
            "#,
        )?;

        let rows = stmt.query_map(params![bundle_id, start_ts, end_ts], |row| {
            let chars: i64 = row.get(1)?;
            Ok(DomainStats {
                domain: row.get(0)?,
                total_chars: chars as u64,
                total_words: row.get::<_, i64>(2)? as u64,
                percentage: if browser_total > 0 {
                    (chars as f64 / browser_total as f64) * 100.0
                } else {
                    0.0
                },
            })
        })?;

        rows.collect::<SqliteResult<Vec<_>>>().map_err(DbError::from)
    }

    /// Get the current active session (if any)
    pub fn get_active_session(&self) -> Result<Option<TypingSession>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, start_time, end_time, char_count, word_count, wpm_avg, wpm_peak
            FROM sessions
            WHERE end_time IS NULL
            ORDER BY start_time DESC
            LIMIT 1
            "#,
        )?;

        let result = stmt.query_row([], |row| {
            let start_ts: i64 = row.get(1)?;
            let end_ts: Option<i64> = row.get(2)?;
            Ok(TypingSession {
                id: Some(row.get(0)?),
                start_time: Utc.timestamp_opt(start_ts, 0).unwrap(),
                end_time: end_ts.map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
                char_count: row.get::<_, i64>(3)? as u32,
                word_count: row.get::<_, i64>(4)? as u32,
                wpm_avg: row.get(5)?,
                wpm_peak: row.get(6)?,
            })
        });

        match result {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::from(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_database_creation() {
        let db = Database::open(":memory:").unwrap();
        assert!(db.get_stats(Utc::now() - Duration::hours(1), Utc::now()).is_ok());
    }

    #[test]
    fn test_keystroke_insert() {
        let db = Database::open(":memory:").unwrap();

        let record = KeystrokeRecord {
            id: None,
            timestamp: Utc::now(),
            app_name: Some("Test App".to_string()),
            app_bundle_id: Some("com.test.app".to_string()),
            char_count: 100,
            word_count: 20,
            paragraph_count: 2,
            backspace_count: 5,
            browser_domain: None,
            browser_url: None,
        };

        let id = db.upsert_keystroke(&record).unwrap();
        assert!(id > 0);

        let stats = db.get_stats(Utc::now() - Duration::hours(1), Utc::now() + Duration::hours(1)).unwrap();
        assert_eq!(stats.total_chars, 100);
        assert_eq!(stats.total_words, 20);
    }
}
