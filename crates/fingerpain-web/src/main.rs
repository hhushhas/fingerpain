//! FingerPain Web Dashboard
//!
//! Local web server for viewing typing statistics with charts.

use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use fingerpain_core::{
    db::Database,
    metrics::{Metrics, TimeRange},
    AggregatedStats, AppStats, HourlyStats, PeakInfo,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing::info;
use url::Url;

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Database>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("fingerpain=info,tower_http=info")
        .init();

    let db = Database::open_default()?;
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/apps", get(apps_handler))
        .route("/api/hourly", get(hourly_handler))
        .route("/api/peak", get(peak_handler))
        .route("/api/daily", get(daily_handler))
        .route("/api/browser-context", post(browser_context_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "127.0.0.1:7890";
    info!("Starting web dashboard at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

#[derive(Deserialize)]
struct RangeQuery {
    range: Option<String>,
}

#[derive(Serialize)]
struct StatsResponse {
    stats: AggregatedStats,
    range: String,
}

async fn stats_handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<StatsResponse>, StatusCode> {
    let range_str = query.range.as_deref().unwrap_or("today");
    let range = TimeRange::parse(range_str).unwrap_or(TimeRange::Today);

    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let metrics = Metrics::new(&*db);
    let stats = metrics.stats(range).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(StatsResponse {
        stats,
        range: range_str.to_string(),
    }))
}

#[derive(Serialize)]
struct AppsResponse {
    apps: Vec<AppStats>,
}

async fn apps_handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<AppsResponse>, StatusCode> {
    let range_str = query.range.as_deref().unwrap_or("week");
    let range = TimeRange::parse(range_str).unwrap_or(TimeRange::ThisWeek);

    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let metrics = Metrics::new(&*db);
    let apps = metrics.app_stats(range).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AppsResponse { apps }))
}

#[derive(Serialize)]
struct HourlyResponse {
    hourly: Vec<HourlyStats>,
}

async fn hourly_handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<HourlyResponse>, StatusCode> {
    let range_str = query.range.as_deref().unwrap_or("month");
    let range = TimeRange::parse(range_str).unwrap_or(TimeRange::ThisMonth);

    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let metrics = Metrics::new(&*db);
    let hourly = metrics.hourly_stats(range).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(HourlyResponse { hourly }))
}

#[derive(Serialize)]
struct PeakResponse {
    peaks: Vec<PeakInfo>,
}

async fn peak_handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<PeakResponse>, StatusCode> {
    let range_str = query.range.as_deref().unwrap_or("month");
    let range = TimeRange::parse(range_str).unwrap_or(TimeRange::ThisMonth);

    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let metrics = Metrics::new(&*db);
    let peaks = metrics.peak_times(range, 10).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(PeakResponse { peaks }))
}

#[derive(Serialize)]
struct DailyDataPoint {
    date: String,
    chars: u64,
    words: u64,
}

#[derive(Serialize)]
struct DailyResponse {
    data: Vec<DailyDataPoint>,
}

#[derive(Deserialize)]
struct BrowserContextRequest {
    url: String,
    title: String,
    browser_name: String,
    timestamp: i64,
}

#[derive(Serialize)]
struct BrowserContextResponse {
    success: bool,
    message: String,
}

async fn daily_handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<DailyResponse>, StatusCode> {
    let range_str = query.range.as_deref().unwrap_or("30d");
    let range = TimeRange::parse(range_str).unwrap_or(TimeRange::Last30Days);

    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let metrics = Metrics::new(&*db);
    let daily = metrics.daily_totals(range).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<DailyDataPoint> = daily
        .into_iter()
        .map(|(date, chars, words)| DailyDataPoint {
            date: date.format("%Y-%m-%d").to_string(),
            chars,
            words,
        })
        .collect();

    Ok(Json(DailyResponse { data }))
}

async fn browser_context_handler(
    State(state): State<AppState>,
    Json(payload): Json<BrowserContextRequest>,
) -> Result<Json<BrowserContextResponse>, StatusCode> {
    // Extract domain from URL
    let domain = match Url::parse(&payload.url) {
        Ok(url) => url.host_str().unwrap_or("unknown").to_string(),
        Err(_) => "unknown".to_string(),
    };

    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Upsert browser context
    db.upsert_browser_context(&payload.browser_name, &payload.url, &domain, &payload.title)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(BrowserContextResponse {
        success: true,
        message: "Context updated".to_string(),
    }))
}
