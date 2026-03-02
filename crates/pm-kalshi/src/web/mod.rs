//! Web dashboard for Kalshi trading

mod garden;
mod handlers;
pub mod ws;

use crate::data::{DataFetcher, FetchState};
use crate::engine::PaperTradingEngine;
use crate::metrics::BacktestResult;
use crate::backtest::BacktestLiveSnapshot;
use axum::routing::{get, post, put};
use axum::Router;
use chrono::{DateTime, Utc};
use pm_store::SqliteStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;

pub use ws::{PipelineMetrics, ServerMessage};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    Idle,
    Paper,
    Backtest,
    Live,
}

impl std::fmt::Display for SessionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Paper => write!(f, "paper"),
            Self::Backtest => write!(f, "backtest"),
            Self::Live => write!(f, "live"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionEnvelope<T> {
    pub mode: SessionMode,
    pub session_id: String,
    #[serde(flatten)]
    pub data: T,
}

impl Default for SessionMode {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub initial_capital: f64,
    pub max_positions: usize,
    pub kelly_fraction: f64,
    pub max_position_pct: f64,
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
    pub max_hold_hours: i64,
    pub min_time_to_close_hours: i64,
    pub max_time_to_close_hours: i64,
    pub cash_reserve_pct: f64,
    pub max_entries_per_tick: usize,
    #[serde(default)]
    pub fees: FeeConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtest_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtest_end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtest_interval_hours: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeeConfig {
    pub taker_rate: f64,
    pub maker_rate: f64,
    pub max_per_contract: f64,
    pub assume_taker: bool,
    pub min_edge_after_fees: f64,
}

#[derive(Debug, Clone)]
pub struct SessionState {
    pub mode: SessionMode,
    pub session_id: String,
    pub config: Option<SessionConfig>,
    pub started_at: Option<DateTime<Utc>>,
    pub trading_active: bool,
}

impl SessionState {
    pub fn new_session(mode: SessionMode, config: SessionConfig) -> Self {
        Self {
            mode,
            session_id: uuid::Uuid::new_v4().to_string(),
            config: Some(config),
            started_at: Some(Utc::now()),
            trading_active: true,
        }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            mode: SessionMode::Idle,
            session_id: String::new(),
            config: None,
            started_at: None,
            trading_active: false,
        }
    }
}

pub enum BacktestRunStatus {
    Idle,
    Running { started_at: DateTime<Utc> },
    Complete,
    Failed,
}

pub struct BacktestProgress {
    pub phase: std::sync::atomic::AtomicU8,
    pub current_step: std::sync::atomic::AtomicU64,
    pub total_steps: std::sync::atomic::AtomicU64,
}

impl BacktestProgress {
    pub const PHASE_LOADING: u8 = 0;
    pub const PHASE_RUNNING: u8 = 1;

    pub fn new(total_steps: u64) -> Self {
        Self {
            phase: std::sync::atomic::AtomicU8::new(Self::PHASE_LOADING),
            current_step: std::sync::atomic::AtomicU64::new(0),
            total_steps: std::sync::atomic::AtomicU64::new(total_steps),
        }
    }

    pub fn phase_name(&self) -> &'static str {
        match self.phase.load(std::sync::atomic::Ordering::Relaxed) {
            Self::PHASE_LOADING => "loading data",
            Self::PHASE_RUNNING => "simulating",
            _ => "unknown",
        }
    }
}

pub struct BacktestState {
    pub status: BacktestRunStatus,
    pub progress: Option<Arc<BacktestProgress>>,
    pub result: Option<BacktestResult>,
    pub error: Option<String>,
    pub live_snapshot: Option<BacktestLiveSnapshot>,
}

#[derive(Debug, Clone)]
pub struct SpecimenInfo {
    pub bed: String,
    pub status: String,
    pub weight: f64,
    pub hit_rate: Option<f64>,
    pub avg_contribution: Option<f64>,
}

impl SpecimenInfo {
    pub fn new(bed: &str, weight: f64) -> Self {
        Self {
            bed: bed.to_string(),
            status: "blooming".to_string(),
            weight,
            hit_rate: None,
            avg_contribution: None,
        }
    }
}

pub struct AppState {
    pub engine: Arc<PaperTradingEngine>,
    pub store: Arc<SqliteStore>,
    pub historical_store: Arc<SqliteStore>,
    pub shutdown_tx: broadcast::Sender<()>,
    pub backtest: Arc<tokio::sync::Mutex<BacktestState>>,
    pub data_dir: PathBuf,
    pub updates_tx: broadcast::Sender<ServerMessage>,
    pub specimens: Arc<RwLock<HashMap<String, SpecimenInfo>>>,
    pub session: Arc<RwLock<SessionState>>,
    pub fetch_state: Arc<RwLock<FetchState>>,
    pub data_fetcher: Arc<DataFetcher>,
    /// Optional path to Becker's prediction-market-analysis data/ directory
    /// for parquet-based backtesting
    pub parquet_data_dir: Option<PathBuf>,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // websocket
        .route("/ws", get(ws::ws_handler))
        // existing REST endpoints
        .route("/api/status", get(handlers::get_status))
        .route("/api/portfolio", get(handlers::get_portfolio))
        .route("/api/positions", get(handlers::get_positions))
        .route("/api/trades", get(handlers::get_trades))
        .route("/api/equity", get(handlers::get_equity))
        .route("/api/circuit-breaker", get(handlers::get_circuit_breaker))
        .route("/api/markets", get(handlers::get_markets))
        .route("/api/control/pause", post(handlers::post_pause))
        .route("/api/control/resume", post(handlers::post_resume))
        .route("/api/backtest/run", post(handlers::post_backtest_run))
        .route("/api/backtest/status", get(handlers::get_backtest_status))
        .route("/api/backtest/result", get(handlers::get_backtest_result))
        .route("/api/backtest/stop", post(handlers::post_backtest_stop))
        // session control
        .route("/api/session/start", post(handlers::post_session_start))
        .route("/api/session/stop", post(handlers::post_session_stop))
        .route("/api/session/config", post(handlers::post_session_config))
        .route("/api/session/status", get(handlers::get_session_status))
        // data fetch endpoints
        .route("/api/data/fetch", post(handlers::post_data_fetch))
        .route("/api/data/status", get(handlers::get_data_status))
        .route("/api/data/available", get(handlers::get_data_available))
        .route("/api/data/cancel", post(handlers::post_data_cancel))
        // garden endpoints
        .route("/api/garden/status", get(garden::get_garden_status))
        .route("/api/beds", get(garden::get_beds))
        .route("/api/beds/{bed}/specimens", get(garden::get_bed_specimens))
        .route(
            "/api/specimens/{name}/status",
            post(garden::post_specimen_status),
        )
        .route(
            "/api/control/scorers/{name}",
            post(garden::post_scorer_toggle),
        )
        .route("/api/control/weights", put(garden::put_weights))
        // static files fallback
        .fallback_service(ServeDir::new("static"))
        .with_state(state)
}

pub fn create_default_specimens() -> HashMap<String, SpecimenInfo> {
    let mut specimens = HashMap::new();

    // momentum bed
    specimens.insert("momentum".to_string(), SpecimenInfo::new("momentum", 0.15));
    specimens.insert(
        "mtf_momentum".to_string(),
        SpecimenInfo::new("momentum", 0.10),
    );
    specimens.insert(
        "time_decay".to_string(),
        SpecimenInfo::new("momentum", 0.10),
    );

    // mean_reversion bed
    specimens.insert(
        "mean_reversion".to_string(),
        SpecimenInfo::new("mean_reversion", 0.15),
    );
    specimens.insert(
        "bollinger".to_string(),
        SpecimenInfo::new("mean_reversion", 0.10),
    );

    // volume bed
    specimens.insert("volume".to_string(), SpecimenInfo::new("volume", 0.10));
    specimens.insert("order_flow".to_string(), SpecimenInfo::new("volume", 0.10));

    // ensemble bed
    specimens.insert(
        "category_weighted".to_string(),
        SpecimenInfo::new("ensemble", 0.20),
    );

    specimens
}
