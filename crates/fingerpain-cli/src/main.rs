//! FingerPain CLI
//!
//! Command-line interface for viewing typing statistics.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Parser, Subcommand};
use colored::Colorize;
use fingerpain_core::{
    db::Database,
    export::{ExportFormat, Exporter},
    metrics::{Metrics, TimeRange},
};
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use tabled::{settings::Style, Table, Tabled};

#[derive(Parser)]
#[command(name = "fingerpain")]
#[command(about = "Typing analytics tracker - see how much you type!")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show today's typing statistics
    Today,

    /// Show yesterday's typing statistics
    Yesterday,

    /// Show this week's typing statistics
    Week,

    /// Show this month's typing statistics
    Month,

    /// Show this year's typing statistics
    Year,

    /// Show statistics for a custom date range
    Range {
        /// Start date (YYYY-MM-DD)
        start: String,
        /// End date (YYYY-MM-DD)
        end: String,
    },

    /// Show peak typing times
    Peak {
        /// Number of peak times to show
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Time range (today, week, month, year, all)
        #[arg(short, long, default_value = "month")]
        range: String,
    },

    /// Show per-app typing breakdown
    Apps {
        /// Time range (today, week, month, year, all)
        #[arg(short, long, default_value = "week")]
        range: String,
    },

    /// Export data to CSV or JSON
    Export {
        /// Output format (csv or json)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Time range (today, week, month, year, all)
        #[arg(short, long, default_value = "all")]
        range: String,

        /// Export only summary (no raw records)
        #[arg(long)]
        summary: bool,
    },

    /// Show daemon status
    Status,

    /// Start the daemon
    Start,

    /// Stop the daemon
    Stop,
}

#[derive(Tabled)]
struct StatRow {
    #[tabled(rename = "Metric")]
    metric: String,
    #[tabled(rename = "Value")]
    value: String,
}

#[derive(Tabled)]
struct AppRow {
    #[tabled(rename = "App")]
    app: String,
    #[tabled(rename = "Characters")]
    chars: String,
    #[tabled(rename = "Words")]
    words: String,
    #[tabled(rename = "%")]
    percentage: String,
}

#[derive(Tabled)]
struct PeakRow {
    #[tabled(rename = "Time")]
    time: String,
    #[tabled(rename = "Characters")]
    chars: String,
    #[tabled(rename = "Words")]
    words: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Open database
    let db = Database::open_default()?;
    let metrics = Metrics::new(&db);

    match cli.command {
        Commands::Today => show_stats(&metrics, TimeRange::Today, "Today"),
        Commands::Yesterday => show_stats(&metrics, TimeRange::Yesterday, "Yesterday"),
        Commands::Week => show_stats(&metrics, TimeRange::ThisWeek, "This Week"),
        Commands::Month => show_stats(&metrics, TimeRange::ThisMonth, "This Month"),
        Commands::Year => show_stats(&metrics, TimeRange::ThisYear, "This Year"),

        Commands::Range { start, end } => {
            let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")?;
            let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")?;

            let start_dt = DateTime::<Utc>::from_naive_utc_and_offset(
                start_date.and_hms_opt(0, 0, 0).unwrap(),
                Utc,
            );
            let end_dt = DateTime::<Utc>::from_naive_utc_and_offset(
                end_date.and_hms_opt(23, 59, 59).unwrap(),
                Utc,
            );

            let range = TimeRange::Custom {
                start: start_dt,
                end: end_dt,
            };
            show_stats(&metrics, range, &format!("{} to {}", start, end))
        }

        Commands::Peak { limit, range } => {
            let time_range = TimeRange::parse(&range).unwrap_or(TimeRange::ThisMonth);
            show_peak(&metrics, time_range, limit)
        }

        Commands::Apps { range } => {
            let time_range = TimeRange::parse(&range).unwrap_or(TimeRange::ThisWeek);
            show_apps(&metrics, time_range)
        }

        Commands::Export {
            format,
            output,
            range,
            summary,
        } => {
            let time_range = TimeRange::parse(&range).unwrap_or(TimeRange::AllTime);
            let export_format = ExportFormat::from_str(&format).unwrap_or(ExportFormat::Json);
            let exporter = Exporter::new(&db);

            let writer: Box<dyn Write> = match output {
                Some(path) => Box::new(File::create(path)?),
                None => Box::new(io::stdout()),
            };

            if summary {
                exporter.export_summary(writer, time_range, export_format)?;
            } else {
                exporter.export(writer, time_range, export_format)?;
            }

            Ok(())
        }

        Commands::Status => show_daemon_status(),
        Commands::Start => start_daemon(),
        Commands::Stop => stop_daemon(),
    }
}

fn show_stats(metrics: &Metrics, range: TimeRange, label: &str) -> Result<()> {
    let stats = metrics.stats(range)?;

    println!("\n{}", format!("📊 {} Statistics", label).bold().cyan());
    println!("{}", "─".repeat(40));

    let rows = vec![
        StatRow {
            metric: "Characters".to_string(),
            value: format!("{} (net: {})",
                Metrics::format_chars(stats.total_chars),
                stats.net_chars
            ),
        },
        StatRow {
            metric: "Words".to_string(),
            value: Metrics::format_words(stats.total_words),
        },
        StatRow {
            metric: "Paragraphs".to_string(),
            value: stats.total_paragraphs.to_string(),
        },
        StatRow {
            metric: "Backspaces".to_string(),
            value: stats.total_backspaces.to_string(),
        },
        StatRow {
            metric: "Active Time".to_string(),
            value: Metrics::format_duration(stats.active_minutes),
        },
        StatRow {
            metric: "Avg WPM".to_string(),
            value: stats
                .avg_wpm
                .map(|w| format!("{:.1}", w))
                .unwrap_or_else(|| "-".to_string()),
        },
        StatRow {
            metric: "Peak WPM".to_string(),
            value: stats
                .peak_wpm
                .map(|w| format!("{:.1}", w))
                .unwrap_or_else(|| "-".to_string()),
        },
    ];

    let table = Table::new(rows).with(Style::rounded()).to_string();
    println!("{}", table);

    Ok(())
}

fn show_apps(metrics: &Metrics, range: TimeRange) -> Result<()> {
    let apps = metrics.app_stats(range)?;

    if apps.is_empty() {
        println!("\n{}", "No app data available for this period.".yellow());
        return Ok(());
    }

    println!("\n{}", "📱 App Breakdown".bold().cyan());
    println!("{}", "─".repeat(60));

    let rows: Vec<AppRow> = apps
        .into_iter()
        .take(10)
        .map(|app| AppRow {
            app: app.app_name,
            chars: Metrics::format_chars(app.total_chars),
            words: Metrics::format_words(app.total_words),
            percentage: format!("{:.1}%", app.percentage),
        })
        .collect();

    let table = Table::new(rows).with(Style::rounded()).to_string();
    println!("{}", table);

    Ok(())
}

fn show_peak(metrics: &Metrics, range: TimeRange, limit: usize) -> Result<()> {
    let peaks = metrics.peak_times(range, limit)?;

    if peaks.is_empty() {
        println!("\n{}", "No peak data available for this period.".yellow());
        return Ok(());
    }

    println!("\n{}", "🔥 Peak Typing Times".bold().cyan());
    println!("{}", "─".repeat(50));

    let rows: Vec<PeakRow> = peaks
        .into_iter()
        .map(|peak| PeakRow {
            time: peak.timestamp.format("%Y-%m-%d %H:%M").to_string(),
            chars: peak.char_count.to_string(),
            words: peak.word_count.to_string(),
        })
        .collect();

    let table = Table::new(rows).with(Style::rounded()).to_string();
    println!("{}", table);

    Ok(())
}

fn show_daemon_status() -> Result<()> {
    // Check if daemon is running by looking for PID file or process
    #[cfg(unix)]
    {
        use std::process::Command;

        let output = Command::new("pgrep")
            .args(["-f", "fingerpain-daemon"])
            .output();

        match output {
            Ok(out) if !out.stdout.is_empty() => {
                let pid = String::from_utf8_lossy(&out.stdout);
                println!("{} (PID: {})", "✓ Daemon is running".green(), pid.trim());
            }
            _ => {
                println!("{}", "✗ Daemon is not running".red());
            }
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;

        let output = Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq fingerpain-daemon.exe"])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains("fingerpain-daemon.exe") {
                    println!("{}", "✓ Daemon is running".green());
                } else {
                    println!("{}", "✗ Daemon is not running".red());
                }
            }
            Err(_) => {
                println!("{}", "✗ Daemon is not running".red());
            }
        }
    }

    Ok(())
}

fn start_daemon() -> Result<()> {
    #[cfg(unix)]
    {
        use std::process::Command;

        // Check if already running
        let check = Command::new("pgrep")
            .args(["-f", "fingerpain-daemon"])
            .output();

        if let Ok(out) = check {
            if !out.stdout.is_empty() {
                println!("{}", "Daemon is already running".yellow());
                return Ok(());
            }
        }

        // Start the daemon
        let result = Command::new("fingerpain-daemon")
            .spawn();

        match result {
            Ok(_) => println!("{}", "✓ Daemon started".green()),
            Err(e) => println!("{}: {}", "✗ Failed to start daemon".red(), e),
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;

        let result = Command::new("cmd")
            .args(["/C", "start", "/B", "fingerpain-daemon.exe"])
            .spawn();

        match result {
            Ok(_) => println!("{}", "✓ Daemon started".green()),
            Err(e) => println!("{}: {}", "✗ Failed to start daemon".red(), e),
        }
    }

    Ok(())
}

fn stop_daemon() -> Result<()> {
    #[cfg(unix)]
    {
        use std::process::Command;

        let result = Command::new("pkill")
            .args(["-f", "fingerpain-daemon"])
            .output();

        match result {
            Ok(out) if out.status.success() => {
                println!("{}", "✓ Daemon stopped".green());
            }
            _ => {
                println!("{}", "Daemon was not running".yellow());
            }
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;

        let result = Command::new("taskkill")
            .args(["/IM", "fingerpain-daemon.exe", "/F"])
            .output();

        match result {
            Ok(out) if out.status.success() => {
                println!("{}", "✓ Daemon stopped".green());
            }
            _ => {
                println!("{}", "Daemon was not running".yellow());
            }
        }
    }

    Ok(())
}
