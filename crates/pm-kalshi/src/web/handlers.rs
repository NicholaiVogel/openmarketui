//! REST API handlers

use super::{AppState, BacktestRunStatus, SessionConfig, SessionMode};
use crate::backtest::Backtester;
use crate::data::HistoricalData;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use pm_core::{BacktestConfig, ExitConfig};
use pm_engine::PositionSizingConfig;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Serialize)]
pub struct StatusResponse {
    pub state: String,
    pub uptime_secs: u64,
    pub last_tick: Option<String>,
    pub ticks_completed: u64,
}

#[derive(Serialize)]
pub struct PortfolioResponse {
    pub cash: f64,
    pub equity: f64,
    pub initial_capital: f64,
    pub return_pct: f64,
    pub drawdown_pct: f64,
    pub positions_count: usize,
}

#[derive(Serialize)]
pub struct PositionResponse {
    pub ticker: String,
    pub title: String,
    pub category: String,
    pub side: String,
    pub quantity: u64,
    pub entry_price: f64,
    pub current_price: Option<f64>,
    pub entry_time: String,
    pub close_time: Option<String>,
    pub unrealized_pnl: f64,
    pub pnl_pct: f64,
    pub hours_held: i64,
}

#[derive(Serialize)]
pub struct TradeResponse {
    pub ticker: String,
    pub side: String,
    pub quantity: u64,
    pub price: f64,
    pub timestamp: String,
    pub fee: Option<f64>,
    pub pnl: Option<f64>,
    pub exit_reason: Option<String>,
}

#[derive(Serialize)]
pub struct EquityPoint {
    pub timestamp: String,
    pub equity: f64,
    pub cash: f64,
    pub positions_value: f64,
    pub drawdown_pct: f64,
}

#[derive(Serialize)]
pub struct CbResponse {
    pub status: String,
    pub events: Vec<CbEventResponse>,
}

#[derive(Serialize)]
pub struct CbEventResponse {
    pub timestamp: String,
    pub rule: String,
    pub details: String,
    pub action: String,
}

pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let status = state.engine.get_status().await;
    Json(StatusResponse {
        state: format!("{}", status.state),
        uptime_secs: status.uptime_secs,
        last_tick: status.last_tick.map(|t| t.to_rfc3339()),
        ticks_completed: status.ticks_completed,
    })
}

pub async fn get_portfolio(State(state): State<Arc<AppState>>) -> Json<PortfolioResponse> {
    let ctx = state.engine.get_context().await;
    let portfolio = &ctx.portfolio;

    let positions_value: f64 = portfolio
        .positions
        .values()
        .map(|p| p.avg_entry_price.to_f64().unwrap_or(0.0) * p.quantity as f64)
        .sum();

    let cash = portfolio.cash.to_f64().unwrap_or(0.0);
    let equity = cash + positions_value;
    let initial = portfolio.initial_capital.to_f64().unwrap_or(10000.0);
    let return_pct = if initial > 0.0 {
        (equity - initial) / initial * 100.0
    } else {
        0.0
    };

    let peak = state
        .store
        .get_peak_equity()
        .await
        .ok()
        .flatten()
        .and_then(|p| p.to_f64())
        .unwrap_or(equity);

    let drawdown_pct = if peak > 0.0 {
        ((peak - equity) / peak * 100.0).max(0.0)
    } else {
        0.0
    };

    Json(PortfolioResponse {
        cash,
        equity,
        initial_capital: initial,
        return_pct,
        drawdown_pct,
        positions_count: portfolio.positions.len(),
    })
}

pub async fn get_positions(State(state): State<Arc<AppState>>) -> Json<Vec<PositionResponse>> {
    let ctx = state.engine.get_context().await;
    let current_prices = state.engine.get_current_prices().await;
    let now = Utc::now();

    let positions: Vec<PositionResponse> = ctx
        .portfolio
        .positions
        .values()
        .map(|p| {
            let entry = p.avg_entry_price.to_f64().unwrap_or(0.0);
            let current = current_prices.get(&p.ticker).and_then(|d| d.to_f64());

            let (unrealized_pnl, pnl_pct) = if let Some(curr) = current {
                let effective_curr = match p.side {
                    pm_core::Side::Yes => curr,
                    pm_core::Side::No => 1.0 - curr,
                };
                let pnl = (effective_curr - entry) * p.quantity as f64;
                let pct = if entry > 0.0 {
                    (effective_curr - entry) / entry * 100.0
                } else {
                    0.0
                };
                (pnl, pct)
            } else {
                (0.0, 0.0)
            };

            let hours_held = (now - p.entry_time).num_hours();

            PositionResponse {
                ticker: p.ticker.clone(),
                title: p.title.clone(),
                category: p.category.clone(),
                side: format!("{:?}", p.side),
                quantity: p.quantity,
                entry_price: entry,
                current_price: current,
                entry_time: p.entry_time.to_rfc3339(),
                close_time: p.close_time.map(|t| t.to_rfc3339()),
                unrealized_pnl,
                pnl_pct,
                hours_held,
            }
        })
        .collect();

    Json(positions)
}

pub async fn get_trades(State(state): State<Arc<AppState>>) -> Json<Vec<TradeResponse>> {
    let fills = state.store.get_recent_fills(100).await.unwrap_or_default();

    let trades: Vec<TradeResponse> = fills
        .into_iter()
        .map(|f| TradeResponse {
            ticker: f.ticker,
            side: format!("{:?}", f.side),
            quantity: f.quantity,
            price: f.price.to_f64().unwrap_or(0.0),
            timestamp: f.timestamp.to_rfc3339(),
            fee: f.fee.and_then(|fee| fee.to_f64()),
            pnl: f.pnl.and_then(|p| p.to_f64()),
            exit_reason: f.exit_reason,
        })
        .collect();

    Json(trades)
}

pub async fn get_equity(State(state): State<Arc<AppState>>) -> Json<Vec<EquityPoint>> {
    let snapshots = state.store.get_equity_curve().await.unwrap_or_default();

    let points: Vec<EquityPoint> = snapshots
        .into_iter()
        .map(|s| EquityPoint {
            timestamp: s.timestamp.to_rfc3339(),
            equity: s.equity.to_f64().unwrap_or(0.0),
            cash: s.cash.to_f64().unwrap_or(0.0),
            positions_value: s.positions_value.to_f64().unwrap_or(0.0),
            drawdown_pct: s.drawdown_pct,
        })
        .collect();

    Json(points)
}

pub async fn get_circuit_breaker(State(state): State<Arc<AppState>>) -> Json<CbResponse> {
    let engine_status = state.engine.get_status().await;
    let cb_status = match engine_status.state {
        crate::engine::EngineState::Paused(ref reason) => format!("tripped: {}", reason),
        _ => "ok".to_string(),
    };

    let events = state
        .store
        .get_circuit_breaker_events(20)
        .await
        .unwrap_or_default();

    let event_responses: Vec<CbEventResponse> = events
        .into_iter()
        .map(|e| CbEventResponse {
            timestamp: e.timestamp.to_rfc3339(),
            rule: e.rule,
            details: e.details,
            action: e.action,
        })
        .collect();

    Json(CbResponse {
        status: cb_status,
        events: event_responses,
    })
}

pub async fn post_pause(State(state): State<Arc<AppState>>) -> StatusCode {
    state.engine.pause("manual pause via API".to_string()).await;
    StatusCode::OK
}

pub async fn post_resume(State(state): State<Arc<AppState>>) -> StatusCode {
    state.engine.resume().await;
    StatusCode::OK
}

#[derive(Deserialize)]
pub struct BacktestRequest {
    pub start: String,
    pub end: String,
    pub capital: Option<f64>,
    pub max_positions: Option<usize>,
    pub max_position: Option<u64>,
    pub interval_hours: Option<i64>,
    pub kelly_fraction: Option<f64>,
    pub max_position_pct: Option<f64>,
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
    pub max_hold_hours: Option<i64>,
}

#[derive(Serialize)]
pub struct BacktestStatusResponse {
    pub status: String,
    pub elapsed_secs: Option<u64>,
    pub error: Option<String>,
    pub phase: Option<String>,
    pub current_step: Option<u64>,
    pub total_steps: Option<u64>,
    pub progress_pct: Option<f64>,
    pub live_snapshot: Option<BacktestLiveSnapshotResponse>,
}

#[derive(Serialize)]
pub struct BacktestLiveSnapshotResponse {
    pub cash: f64,
    pub invested: f64,
    pub equity: f64,
    pub initial_capital: f64,
    pub return_pct: f64,
    pub total_pnl: f64,
    pub open_positions: usize,
    pub fills_this_step: usize,
}

#[derive(Serialize)]
pub struct BacktestErrorResponse {
    pub error: String,
}

pub async fn post_backtest_run(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BacktestRequest>,
) -> Result<StatusCode, (StatusCode, Json<BacktestErrorResponse>)> {
    {
        let guard = state.backtest.lock().await;
        if matches!(guard.status, BacktestRunStatus::Running { .. }) {
            return Err((
                StatusCode::CONFLICT,
                Json(BacktestErrorResponse {
                    error: "backtest already running".into(),
                }),
            ));
        }
    }

    let start_time = crate::parse_date(&req.start).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestErrorResponse {
                error: format!("invalid start date: {}", e),
            }),
        )
    })?;
    let end_time = crate::parse_date(&req.end).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestErrorResponse {
                error: format!("invalid end date: {}", e),
            }),
        )
    })?;

    info!(
        start = %start_time,
        end = %end_time,
        "starting backtest from web UI"
    );

    let progress = Arc::new(super::BacktestProgress::new(0));

    {
        let mut guard = state.backtest.lock().await;
        guard.status = BacktestRunStatus::Running {
            started_at: Utc::now(),
        };
        guard.progress = Some(progress.clone());
        guard.result = None;
        guard.error = None;
        guard.live_snapshot = None;
    }

    let backtest_state = state.backtest.clone();
    let session_state = state.session.clone();
    let progress = progress.clone();

    let capital = req.capital.unwrap_or(10000.0);
    let max_positions = req.max_positions.unwrap_or(100);
    let max_position = req.max_position.unwrap_or(100);
    let interval_hours = req.interval_hours.unwrap_or(1);
    let kelly_fraction = req.kelly_fraction.unwrap_or(0.40);
    let max_position_pct = req.max_position_pct.unwrap_or(0.30);
    let take_profit = req.take_profit.unwrap_or(0.50);
    let stop_loss = req.stop_loss.unwrap_or(0.99);
    let max_hold_hours = req.max_hold_hours.unwrap_or(48);

    let historical_store = state.historical_store.clone();

    tokio::spawn(async move {
        let data = {
            info!("loading backtest data from historical sqlite");
            match HistoricalData::load_sqlite(&historical_store, start_time, end_time).await {
                Ok(d) => Arc::new(d),
                Err(e) => {
                    let mut guard = backtest_state.lock().await;
                    guard.status = BacktestRunStatus::Failed;
                    guard.error = Some(format!("failed to load data from sqlite: {}", e));
                    error!(error = %e, "backtest sqlite load failed");

                    let mut session = session_state.write().await;
                    if session.mode == SessionMode::Backtest {
                        *session = super::SessionState::default();
                    }
                    return;
                }
            }
        };

        let config = BacktestConfig {
            start_time,
            end_time,
            interval: chrono::TimeDelta::hours(interval_hours),
            initial_capital: Decimal::try_from(capital).unwrap_or(Decimal::new(10000, 0)),
            max_position_size: max_position,
            max_positions,
        };

        let sizing_config = PositionSizingConfig {
            kelly_fraction,
            max_position_pct,
            min_position_size: 10,
            max_position_size: max_position,
        };

        let exit_config = ExitConfig {
            take_profit_pct: take_profit,
            stop_loss_pct: stop_loss,
            max_hold_hours,
            score_reversal_threshold: -0.3,
        };

        let backtest_state_for_live = backtest_state.clone();
        let backtester = Backtester::with_configs(config, data, sizing_config, exit_config)
            .with_progress(progress)
            .with_step_callback(move |snapshot| {
                if let Ok(mut guard) = backtest_state_for_live.try_lock() {
                    guard.live_snapshot = Some(snapshot);
                }
            });
        let result = backtester.run().await;

        let mut guard = backtest_state.lock().await;
        guard.status = BacktestRunStatus::Complete;
        guard.result = Some(result);

        let mut session = session_state.write().await;
        if session.mode == SessionMode::Backtest {
            *session = super::SessionState::default();
        }
    });

    Ok(StatusCode::OK)
}

pub async fn get_backtest_status(
    State(state): State<Arc<AppState>>,
) -> Json<BacktestStatusResponse> {
    let guard = state.backtest.lock().await;
    let (status_str, elapsed, error) = match &guard.status {
        BacktestRunStatus::Idle => ("idle".to_string(), None, None),
        BacktestRunStatus::Running { started_at } => {
            let elapsed = Utc::now()
                .signed_duration_since(*started_at)
                .num_seconds()
                .max(0) as u64;
            ("running".to_string(), Some(elapsed), None)
        }
        BacktestRunStatus::Complete => ("complete".to_string(), None, None),
        BacktestRunStatus::Failed => ("failed".to_string(), None, guard.error.clone()),
    };

    let (phase, current_step, total_steps, progress_pct) = if let Some(ref p) = guard.progress {
        let current = p.current_step.load(std::sync::atomic::Ordering::Relaxed);
        let total = p.total_steps.load(std::sync::atomic::Ordering::Relaxed);
        let pct = if total > 0 {
            current as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        (
            Some(p.phase_name().to_string()),
            Some(current),
            Some(total),
            Some(pct),
        )
    } else {
        (None, None, None, None)
    };

    let live_snapshot = guard
        .live_snapshot
        .clone()
        .map(|s| BacktestLiveSnapshotResponse {
            cash: s.cash,
            invested: s.invested,
            equity: s.equity,
            initial_capital: s.initial_capital,
            return_pct: s.return_pct,
            total_pnl: s.total_pnl,
            open_positions: s.open_positions,
            fills_this_step: s.fills_this_step,
        });

    Json(BacktestStatusResponse {
        status: status_str,
        elapsed_secs: elapsed,
        error,
        phase,
        current_step,
        total_steps,
        progress_pct,
        live_snapshot,
    })
}

pub async fn get_backtest_result(
    State(state): State<Arc<AppState>>,
) -> Result<Json<crate::metrics::BacktestResult>, StatusCode> {
    let guard = state.backtest.lock().await;
    match &guard.result {
        Some(result) => Ok(Json(result.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn post_backtest_stop(State(state): State<Arc<AppState>>) -> StatusCode {
    {
        let mut guard = state.backtest.lock().await;
        // Reset backtest state
        guard.status = BacktestRunStatus::Idle;
        guard.error = None;
        guard.progress = None;
        guard.live_snapshot = None;
        // Keep result if there was one
    }

    // Also reset session state
    {
        let mut session = state.session.write().await;
        if session.mode == super::SessionMode::Backtest {
            *session = super::SessionState::default();
        }
    }

    info!("backtest stopped/reset via API");
    StatusCode::OK
}

#[derive(Serialize)]
pub struct SessionStatusResponse {
    pub mode: SessionMode,
    pub session_id: String,
    pub trading_active: bool,
    pub started_at: Option<String>,
    pub config: Option<SessionConfig>,
}

pub async fn get_session_status(State(state): State<Arc<AppState>>) -> Json<SessionStatusResponse> {
    let session = state.session.read().await;
    Json(SessionStatusResponse {
        mode: session.mode.clone(),
        session_id: session.session_id.clone(),
        trading_active: session.trading_active,
        started_at: session.started_at.map(|t| t.to_rfc3339()),
        config: session.config.clone(),
    })
}

#[derive(Deserialize)]
pub struct SessionStartRequest {
    pub mode: SessionMode,
    pub config: SessionConfig,
}

#[derive(Serialize)]
pub struct SessionErrorResponse {
    pub error: String,
}

pub async fn post_session_start(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SessionStartRequest>,
) -> Result<StatusCode, (StatusCode, Json<SessionErrorResponse>)> {
    {
        let session = state.session.read().await;
        if session.trading_active {
            return Err((
                StatusCode::CONFLICT,
                Json(SessionErrorResponse {
                    error: "session already running".into(),
                }),
            ));
        }
    }

    match req.mode {
        SessionMode::Paper => {
            {
                let mut session = state.session.write().await;
                *session = super::SessionState::new_session(SessionMode::Paper, req.config);
            }

            state.engine.resume().await;
            info!("paper trading session started via API");
            Ok(StatusCode::OK)
        }
        SessionMode::Backtest => {
            let config = req.config;
            let start = config.backtest_start.clone().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(SessionErrorResponse {
                        error: "backtest_start required for backtest mode".into(),
                    }),
                )
            })?;
            let end = config.backtest_end.clone().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(SessionErrorResponse {
                        error: "backtest_end required for backtest mode".into(),
                    }),
                )
            })?;

            let backtest_req = BacktestRequest {
                start,
                end,
                capital: Some(config.initial_capital),
                max_positions: Some(config.max_positions),
                max_position: Some((config.initial_capital * config.max_position_pct) as u64),
                interval_hours: config.backtest_interval_hours,
                kelly_fraction: Some(config.kelly_fraction),
                max_position_pct: Some(config.max_position_pct),
                take_profit: Some(config.take_profit_pct),
                stop_loss: Some(config.stop_loss_pct),
                max_hold_hours: Some(config.max_hold_hours),
            };

            let state_for_backtest = state.clone();
            post_backtest_run(State(state_for_backtest), Json(backtest_req))
                .await
                .map_err(|(code, Json(e))| {
                    (
                        code,
                        Json(SessionErrorResponse {
                            error: e.error.clone(),
                        }),
                    )
                })?;

            {
                let mut session = state.session.write().await;
                *session = super::SessionState::new_session(SessionMode::Backtest, config);
            }
            Ok(StatusCode::OK)
        }
        SessionMode::Live => Err((
            StatusCode::NOT_IMPLEMENTED,
            Json(SessionErrorResponse {
                error: "live trading not yet implemented".into(),
            }),
        )),
        SessionMode::Idle => {
            let mut session = state.session.write().await;
            session.mode = SessionMode::Idle;
            session.trading_active = false;
            Ok(StatusCode::OK)
        }
    }
}

pub async fn post_session_stop(State(state): State<Arc<AppState>>) -> StatusCode {
    {
        let mut session = state.session.write().await;
        *session = super::SessionState::default();
    }

    state
        .engine
        .pause("session stopped via API".to_string())
        .await;
    info!("trading session stopped via API");
    StatusCode::OK
}

pub async fn post_session_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<SessionConfig>,
) -> StatusCode {
    let mut session = state.session.write().await;
    session.config = Some(config);
    info!("session config updated via API");
    StatusCode::OK
}

// data fetch handlers

#[derive(Deserialize)]
pub struct DataFetchRequest {
    pub start_date: String,
    pub end_date: String,
    #[serde(default = "default_trades_per_day")]
    pub trades_per_day: usize,
    #[serde(default = "default_fetch_markets")]
    pub fetch_markets: bool,
    #[serde(default = "default_fetch_trades")]
    pub fetch_trades: bool,
}

fn default_trades_per_day() -> usize {
    100_000
}

fn default_fetch_markets() -> bool {
    true
}

fn default_fetch_trades() -> bool {
    true
}

#[derive(Serialize)]
pub struct DataFetchResponse {
    pub success: bool,
    pub message: String,
}

pub async fn post_data_fetch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DataFetchRequest>,
) -> Result<Json<DataFetchResponse>, (StatusCode, Json<DataFetchResponse>)> {
    use crate::data::FetchStatus;
    use std::sync::atomic::Ordering;

    {
        let guard = state.fetch_state.read().await;
        if guard.status == FetchStatus::Fetching {
            return Err((
                StatusCode::CONFLICT,
                Json(DataFetchResponse {
                    success: false,
                    message: "data fetch already in progress".into(),
                }),
            ));
        }
    }

    let start = chrono::NaiveDate::parse_from_str(&req.start_date, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(DataFetchResponse {
                success: false,
                message: format!("invalid start date: {}", e),
            }),
        )
    })?;

    let end = chrono::NaiveDate::parse_from_str(&req.end_date, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(DataFetchResponse {
                success: false,
                message: format!("invalid end date: {}", e),
            }),
        )
    })?;

    let fetch_markets = req.fetch_markets;
    let fetch_trades = req.fetch_trades;

    if !fetch_markets && !fetch_trades {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(DataFetchResponse {
                success: false,
                message: "at least one of fetch_markets or fetch_trades must be true".into(),
            }),
        ));
    }

    {
        let mut guard = state.fetch_state.write().await;
        guard.status = FetchStatus::Fetching;
        guard.cancel_requested.store(false, Ordering::Relaxed);
        guard.error = None;
    }

    let fetcher = state.data_fetcher.clone();
    let fetch_state = state.fetch_state.clone();
    let trades_per_day = req.trades_per_day;

    tokio::spawn(async move {
        match fetcher
            .fetch_range(
                start,
                end,
                trades_per_day,
                fetch_markets,
                fetch_trades,
                fetch_state.clone(),
            )
            .await
        {
            Ok(()) => {
                info!("data fetch completed successfully");
            }
            Err(e) => {
                error!(error = %e, "data fetch failed");
                let mut guard = fetch_state.write().await;
                guard.status = FetchStatus::Failed;
                guard.error = Some(e.to_string());
            }
        }
    });

    Ok(Json(DataFetchResponse {
        success: true,
        message: "data fetch started".into(),
    }))
}

pub async fn get_data_status(
    State(state): State<Arc<AppState>>,
) -> Json<crate::data::FetchProgress> {
    let guard = state.fetch_state.read().await;
    Json(guard.to_progress())
}

pub async fn get_data_available(
    State(state): State<Arc<AppState>>,
) -> Result<Json<crate::data::DataAvailability>, StatusCode> {
    match state.data_fetcher.get_available_data().await {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            error!(error = %e, "failed to get available data");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn post_data_cancel(State(state): State<Arc<AppState>>) -> StatusCode {
    use std::sync::atomic::Ordering;

    let guard = state.fetch_state.read().await;
    guard.cancel_requested.store(true, Ordering::Relaxed);
    info!("data fetch cancellation requested");
    StatusCode::OK
}

// markets endpoint

#[derive(Serialize)]
pub struct MarketResponse {
    pub ticker: String,
    pub title: String,
    pub category: Option<String>,
    pub status: String,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub volume_24h: Option<f64>,
    pub in_watchlist: bool,
}

#[derive(Deserialize)]
pub struct MarketsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

pub async fn get_markets(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<MarketsQuery>,
) -> Json<Vec<MarketResponse>> {
    let candidates = state.engine.get_last_candidates().await;
    let ctx = state.engine.get_context().await;

    let markets: Vec<MarketResponse> = candidates
        .into_iter()
        .take(query.limit)
        .map(|c| {
            let has_position = ctx.portfolio.has_position(&c.ticker);
            MarketResponse {
                ticker: c.ticker,
                title: c.title,
                category: if c.category.is_empty() {
                    None
                } else {
                    Some(c.category)
                },
                status: "open".to_string(),
                yes_price: c.current_yes_price.to_f64(),
                no_price: c.current_no_price.to_f64(),
                volume_24h: Some(c.volume_24h as f64),
                in_watchlist: has_position,
            }
        })
        .collect();

    Json(markets)
}
