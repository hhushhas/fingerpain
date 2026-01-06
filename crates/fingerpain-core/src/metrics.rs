//! Metrics aggregation and time range utilities

use crate::{db::Database, AggregatedStats, AppStats, HourlyStats, PeakInfo};
use chrono::{DateTime, Datelike, Duration, NaiveTime, TimeZone, Utc};

/// Time range for querying stats
#[derive(Debug, Clone, Copy)]
pub enum TimeRange {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    ThisYear,
    LastYear,
    Last7Days,
    Last30Days,
    Last90Days,
    AllTime,
    Custom { start: DateTime<Utc>, end: DateTime<Utc> },
}

impl TimeRange {
    /// Convert to start and end timestamps
    pub fn to_range(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        let today_start = now.date_naive().and_time(NaiveTime::MIN);
        let today_start = DateTime::<Utc>::from_naive_utc_and_offset(today_start, Utc);

        match self {
            TimeRange::Today => (today_start, now),

            TimeRange::Yesterday => {
                let yesterday = today_start - Duration::days(1);
                (yesterday, today_start)
            }

            TimeRange::ThisWeek => {
                let days_since_monday = now.weekday().num_days_from_monday() as i64;
                let week_start = today_start - Duration::days(days_since_monday);
                (week_start, now)
            }

            TimeRange::LastWeek => {
                let days_since_monday = now.weekday().num_days_from_monday() as i64;
                let this_week_start = today_start - Duration::days(days_since_monday);
                let last_week_start = this_week_start - Duration::days(7);
                (last_week_start, this_week_start)
            }

            TimeRange::ThisMonth => {
                let month_start = today_start
                    .with_day(1)
                    .unwrap_or(today_start);
                (month_start, now)
            }

            TimeRange::LastMonth => {
                let this_month_start = today_start.with_day(1).unwrap_or(today_start);
                let last_month = if now.month() == 1 {
                    this_month_start.with_year(now.year() - 1).unwrap().with_month(12).unwrap()
                } else {
                    this_month_start.with_month(now.month() - 1).unwrap()
                };
                (last_month, this_month_start)
            }

            TimeRange::ThisYear => {
                let year_start = today_start
                    .with_month(1)
                    .unwrap()
                    .with_day(1)
                    .unwrap();
                (year_start, now)
            }

            TimeRange::LastYear => {
                let this_year_start = today_start
                    .with_month(1)
                    .unwrap()
                    .with_day(1)
                    .unwrap();
                let last_year_start = this_year_start.with_year(now.year() - 1).unwrap();
                (last_year_start, this_year_start)
            }

            TimeRange::Last7Days => (now - Duration::days(7), now),
            TimeRange::Last30Days => (now - Duration::days(30), now),
            TimeRange::Last90Days => (now - Duration::days(90), now),

            TimeRange::AllTime => {
                // Far past to now
                let epoch = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
                (epoch, now)
            }

            TimeRange::Custom { start, end } => (*start, *end),
        }
    }

    /// Parse from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "today" => Some(TimeRange::Today),
            "yesterday" => Some(TimeRange::Yesterday),
            "week" | "this-week" | "thisweek" => Some(TimeRange::ThisWeek),
            "last-week" | "lastweek" => Some(TimeRange::LastWeek),
            "month" | "this-month" | "thismonth" => Some(TimeRange::ThisMonth),
            "last-month" | "lastmonth" => Some(TimeRange::LastMonth),
            "year" | "this-year" | "thisyear" => Some(TimeRange::ThisYear),
            "last-year" | "lastyear" => Some(TimeRange::LastYear),
            "7d" | "7days" | "last7days" => Some(TimeRange::Last7Days),
            "30d" | "30days" | "last30days" => Some(TimeRange::Last30Days),
            "90d" | "90days" | "last90days" | "3months" => Some(TimeRange::Last90Days),
            "all" | "alltime" | "all-time" => Some(TimeRange::AllTime),
            _ => None,
        }
    }
}

/// High-level metrics API
pub struct Metrics<'a> {
    db: &'a Database,
}

impl<'a> Metrics<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Get aggregated statistics for a time range
    pub fn stats(&self, range: TimeRange) -> crate::db::Result<AggregatedStats> {
        let (start, end) = range.to_range();
        self.db.get_stats(start, end)
    }

    /// Get per-app statistics
    pub fn app_stats(&self, range: TimeRange) -> crate::db::Result<Vec<AppStats>> {
        let (start, end) = range.to_range();
        self.db.get_app_stats(start, end)
    }

    /// Get hourly breakdown for heatmap
    pub fn hourly_stats(&self, range: TimeRange) -> crate::db::Result<Vec<HourlyStats>> {
        let (start, end) = range.to_range();
        self.db.get_hourly_stats(start, end)
    }

    /// Get peak typing times
    pub fn peak_times(&self, range: TimeRange, limit: usize) -> crate::db::Result<Vec<PeakInfo>> {
        let (start, end) = range.to_range();
        self.db.get_peak_times(start, end, limit)
    }

    /// Get daily totals for charting
    pub fn daily_totals(&self, range: TimeRange) -> crate::db::Result<Vec<(DateTime<Utc>, u64, u64)>> {
        let (start, end) = range.to_range();
        self.db.get_daily_totals(start, end)
    }

    /// Format character count for display
    pub fn format_chars(count: u64) -> String {
        if count >= 1_000_000 {
            format!("{:.1}M", count as f64 / 1_000_000.0)
        } else if count >= 1_000 {
            format!("{:.1}K", count as f64 / 1_000.0)
        } else {
            count.to_string()
        }
    }

    /// Format word count for display
    pub fn format_words(count: u64) -> String {
        if count >= 1_000_000 {
            format!("{:.1}M", count as f64 / 1_000_000.0)
        } else if count >= 1_000 {
            format!("{:.1}K", count as f64 / 1_000.0)
        } else {
            count.to_string()
        }
    }

    /// Format duration in minutes to human readable
    pub fn format_duration(minutes: u32) -> String {
        if minutes >= 60 {
            let hours = minutes / 60;
            let mins = minutes % 60;
            if mins > 0 {
                format!("{}h {}m", hours, mins)
            } else {
                format!("{}h", hours)
            }
        } else {
            format!("{}m", minutes)
        }
    }
}
