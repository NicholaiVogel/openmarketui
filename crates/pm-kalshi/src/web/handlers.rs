//! REST API handlers

use super::{AppState, BacktestRunStatus, SessionConfig, SessionMode};
use crate::backtest::Backtester;
use crate::data::HistoricalData;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::{DateTime, Utc};
use pm_core::{BacktestConfig, ExitConfig, MarketResult};
use pm_engine::PositionSizingConfig;
use pm_store::{
    AuditEvent, BacktestRunRecord, DecisionRecord, NewAuditEvent, NewBacktestRun, NewSessionRun,
    SessionRunRecord,
};
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
pub struct PositionCloseResponse {
    pub ticker: String,
    pub side: String,
    pub quantity_requested: u64,
    pub quantity_filled: u64,
    pub fill_price: f64,
    pub fee: Option<f64>,
    pub pnl: Option<f64>,
    pub cash_after: f64,
    pub remaining_quantity: u64,
    pub closed: bool,
    pub timestamp: String,
    pub price_source: String,
}

#[derive(Serialize)]
pub struct PositionCloseErrorResponse {
    pub error: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionRedeemOutcome {
    Yes,
    No,
    Cancelled,
}

impl PositionRedeemOutcome {
    fn as_market_result(self) -> MarketResult {
        match self {
            Self::Yes => MarketResult::Yes,
            Self::No => MarketResult::No,
            Self::Cancelled => MarketResult::Cancelled,
        }
    }

    fn from_market_result(result: MarketResult) -> Self {
        match result {
            MarketResult::Yes => Self::Yes,
            MarketResult::No => Self::No,
            MarketResult::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Deserialize, Default)]
pub struct PositionRedeemRequest {
    pub result: Option<PositionRedeemOutcome>,
}

#[derive(Serialize)]
pub struct PositionRedeemResponse {
    pub ticker: String,
    pub side: String,
    pub quantity_redeemed: u64,
    pub settlement_price: f64,
    pub market_result: PositionRedeemOutcome,
    pub payout: f64,
    pub pnl: Option<f64>,
    pub cash_after: f64,
    pub timestamp: String,
    pub result_source: String,
}

#[derive(Serialize)]
pub struct PositionRedeemSkip {
    pub ticker: String,
    pub reason: String,
}

#[derive(Serialize)]
pub struct PositionsRedeemResponse {
    pub redeemed_count: usize,
    pub skipped_count: usize,
    pub redeemed: Vec<PositionRedeemResponse>,
    pub skipped: Vec<PositionRedeemSkip>,
}

#[derive(Serialize)]
pub struct PositionRedeemErrorResponse {
    pub error: String,
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

pub async fn get_snapshot(State(state): State<Arc<AppState>>) -> Json<super::ws::ServerMessage> {
    Json(super::ws::build_snapshot(&state).await)
}

pub async fn post_daemon_shutdown(State(state): State<Arc<AppState>>) -> StatusCode {
    match state.shutdown_tx.send(()) {
        Ok(_) => StatusCode::ACCEPTED,
        Err(e) => {
            error!(error = %e, "failed to send daemon shutdown signal");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[derive(Serialize)]
pub struct AuthStatusResponse {
    pub provider: String,
    pub public_market_data: bool,
    pub credentials_loaded: bool,
    pub live_trading_supported: bool,
    pub live_trading_enabled: bool,
    pub credential_source: Option<String>,
    pub reason: String,
}

pub async fn get_auth_status() -> Json<AuthStatusResponse> {
    Json(AuthStatusResponse {
        provider: "kalshi".to_string(),
        public_market_data: true,
        credentials_loaded: false,
        live_trading_supported: false,
        live_trading_enabled: false,
        credential_source: None,
        reason: "daemon currently uses public Kalshi market data endpoints only; live credential loading is not implemented".to_string(),
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

pub async fn post_position_close(
    State(state): State<Arc<AppState>>,
    Path(ticker): Path<String>,
) -> Result<Json<PositionCloseResponse>, (StatusCode, Json<PositionCloseErrorResponse>)> {
    match state.engine.close_position(&ticker).await {
        Ok(Some(result)) => Ok(Json(PositionCloseResponse {
            ticker: result.ticker,
            side: format!("{:?}", result.side),
            quantity_requested: result.quantity_requested,
            quantity_filled: result.quantity_filled,
            fill_price: result.fill_price.to_f64().unwrap_or(0.0),
            fee: result.fee.and_then(|fee| fee.to_f64()),
            pnl: result.pnl.and_then(|pnl| pnl.to_f64()),
            cash_after: result.cash_after.to_f64().unwrap_or(0.0),
            remaining_quantity: result.remaining_quantity,
            closed: result.closed,
            timestamp: result.timestamp.to_rfc3339(),
            price_source: result.price_source,
        })),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(PositionCloseErrorResponse {
                error: format!("position not found: {ticker}"),
            }),
        )),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PositionCloseErrorResponse {
                error: err.to_string(),
            }),
        )),
    }
}

pub async fn post_positions_redeem(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PositionRedeemRequest>,
) -> Result<Json<PositionsRedeemResponse>, (StatusCode, Json<PositionRedeemErrorResponse>)> {
    if req.result.is_some() {
        return Err(redeem_error(
            StatusCode::BAD_REQUEST,
            "bulk redeem does not accept an explicit result; pass a ticker to redeem manually",
        ));
    }

    let tickers: Vec<String> = {
        let ctx = state.engine.get_context().await;
        ctx.portfolio.positions.keys().cloned().collect()
    };

    let mut redeemed = Vec::new();
    let mut skipped = Vec::new();

    for ticker in tickers {
        let (result, source) = match lookup_redeem_result(&state, &ticker, None).await {
            Ok(resolution) => resolution,
            Err((StatusCode::CONFLICT, Json(err))) => {
                skipped.push(PositionRedeemSkip {
                    ticker,
                    reason: err.error,
                });
                continue;
            }
            Err(err) => return Err(err),
        };

        match state.engine.redeem_position(&ticker, result, source).await {
            Ok(Some(result)) => redeemed.push(position_redeem_response(result)),
            Ok(None) => skipped.push(PositionRedeemSkip {
                ticker,
                reason: "position not found".to_string(),
            }),
            Err(err) => {
                return Err(redeem_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    err.to_string(),
                ))
            }
        }
    }

    Ok(Json(PositionsRedeemResponse {
        redeemed_count: redeemed.len(),
        skipped_count: skipped.len(),
        redeemed,
        skipped,
    }))
}

pub async fn post_position_redeem(
    State(state): State<Arc<AppState>>,
    Path(ticker): Path<String>,
    Json(req): Json<PositionRedeemRequest>,
) -> Result<Json<PositionRedeemResponse>, (StatusCode, Json<PositionRedeemErrorResponse>)> {
    let (result, source) = lookup_redeem_result(&state, &ticker, req.result).await?;
    match state.engine.redeem_position(&ticker, result, source).await {
        Ok(Some(result)) => Ok(Json(position_redeem_response(result))),
        Ok(None) => Err(redeem_error(
            StatusCode::NOT_FOUND,
            format!("position not found: {ticker}"),
        )),
        Err(err) => Err(redeem_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )),
    }
}

async fn lookup_redeem_result(
    state: &Arc<AppState>,
    ticker: &str,
    explicit: Option<PositionRedeemOutcome>,
) -> Result<(MarketResult, String), (StatusCode, Json<PositionRedeemErrorResponse>)> {
    if let Some(result) = explicit {
        return Ok((result.as_market_result(), "manual_request".to_string()));
    }

    let now = Utc::now();
    for candidate in state.engine.get_last_candidates().await {
        if candidate.ticker != ticker || candidate.close_time > now {
            continue;
        }
        if let Some(result) = candidate.result {
            return Ok((result, "daemon_candidates".to_string()));
        }
    }

    match state.historical_store.get_historical_market(ticker).await {
        Ok(Some(row)) => {
            let close_time = row.close_time.parse::<DateTime<Utc>>().map_err(|e| {
                redeem_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to parse historical close_time for {ticker}: {e}"),
                )
            })?;
            if close_time > now {
                return Err(redeem_error(
                    StatusCode::CONFLICT,
                    format!("position is not resolved yet: {ticker}"),
                ));
            }
            if let Some(result) = row.result.as_deref().and_then(parse_market_result) {
                return Ok((result, "historical_store".to_string()));
            }
            Err(redeem_error(
                StatusCode::CONFLICT,
                format!("no resolved result available for {ticker}"),
            ))
        }
        Ok(None) => Err(redeem_error(
            StatusCode::CONFLICT,
            format!("no resolved result available for {ticker}"),
        )),
        Err(err) => Err(redeem_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )),
    }
}

fn parse_market_result(value: &str) -> Option<MarketResult> {
    match value.trim().to_lowercase().as_str() {
        "yes" => Some(MarketResult::Yes),
        "no" => Some(MarketResult::No),
        "cancelled" | "canceled" => Some(MarketResult::Cancelled),
        _ => None,
    }
}

fn position_redeem_response(result: crate::engine::ManualRedeemResult) -> PositionRedeemResponse {
    PositionRedeemResponse {
        ticker: result.ticker,
        side: format!("{:?}", result.side),
        quantity_redeemed: result.quantity_redeemed,
        settlement_price: result.settlement_price.to_f64().unwrap_or(0.0),
        market_result: PositionRedeemOutcome::from_market_result(result.market_result),
        payout: result.payout.to_f64().unwrap_or(0.0),
        pnl: result.pnl.and_then(|pnl| pnl.to_f64()),
        cash_after: result.cash_after.to_f64().unwrap_or(0.0),
        timestamp: result.timestamp.to_rfc3339(),
        result_source: result.result_source,
    }
}

fn redeem_error(
    status: StatusCode,
    error: impl Into<String>,
) -> (StatusCode, Json<PositionRedeemErrorResponse>) {
    (
        status,
        Json(PositionRedeemErrorResponse {
            error: error.into(),
        }),
    )
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
    /// "parquet" to use Becker dataset, "sqlite" (default) for local data
    #[serde(default)]
    pub data_source: Option<String>,
}

#[derive(Serialize)]
pub struct BacktestStatusResponse {
    pub status: String,
    pub run_id: Option<String>,
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

async fn finish_backtest_session(
    store: &Arc<pm_store::SqliteStore>,
    session_state: &Arc<tokio::sync::RwLock<super::SessionState>>,
    status: &str,
    reason: Option<&str>,
) {
    let session_id = {
        let session = session_state.read().await;
        if session.mode == SessionMode::Backtest && !session.session_id.is_empty() {
            Some(session.session_id.clone())
        } else {
            None
        }
    };

    let Some(session_id) = session_id else {
        return;
    };

    if let Err(e) = store.finish_session_run(&session_id, status, reason).await {
        error!(session_id = %session_id, status = %status, error = %e, "failed to persist session end");
    }

    let mut session = session_state.write().await;
    if session.mode == SessionMode::Backtest && session.session_id == session_id {
        *session = super::SessionState::default();
    }
}

async fn mark_backtest_failed(
    store: &Arc<pm_store::SqliteStore>,
    backtest_state: &Arc<tokio::sync::Mutex<super::BacktestState>>,
    session_state: &Arc<tokio::sync::RwLock<super::SessionState>>,
    run_id: &str,
    message: String,
) {
    {
        let mut guard = backtest_state.lock().await;
        guard.status = BacktestRunStatus::Failed;
        guard.error = Some(message.clone());
    }

    if let Err(e) = store
        .finish_backtest_run_with_error(run_id, "failed", &message)
        .await
    {
        error!(run_id = %run_id, error = %e, "failed to persist failed backtest run");
    }

    finish_backtest_session(store, session_state, "failed", Some(&message)).await;
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
    let run_store = state.store.clone();
    let parquet_data_dir = state.parquet_data_dir.clone();
    let use_parquet = req.data_source.as_deref() == Some("parquet") || parquet_data_dir.is_some();
    let data_source = if use_parquet { "parquet" } else { "sqlite" }.to_string();
    let run_id = uuid::Uuid::new_v4().to_string();
    let started_at = Utc::now();

    run_store
        .record_backtest_run_started(&NewBacktestRun {
            run_id: run_id.clone(),
            started_at: started_at.clone(),
            start_time: start_time.clone(),
            end_time: end_time.clone(),
            capital,
            max_positions,
            max_position,
            interval_hours,
            kelly_fraction,
            max_position_pct,
            take_profit,
            stop_loss,
            max_hold_hours,
            data_source: data_source.clone(),
        })
        .await
        .map_err(|e| {
            error!(error = %e, "failed to persist backtest run start");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BacktestErrorResponse {
                    error: "failed to persist backtest run".into(),
                }),
            )
        })?;

    info!(
        run_id = %run_id,
        start = %start_time,
        end = %end_time,
        data_source = %data_source,
        "starting backtest from web UI"
    );

    let progress = Arc::new(super::BacktestProgress::new(0));

    {
        let mut guard = state.backtest.lock().await;
        guard.status = BacktestRunStatus::Running { started_at };
        guard.run_id = Some(run_id.clone());
        guard.progress = Some(progress.clone());
        guard.result = None;
        guard.error = None;
        guard.live_snapshot = None;
    }

    let backtest_state = state.backtest.clone();
    let session_state = state.session.clone();
    let progress = progress.clone();
    let run_id_for_task = run_id.clone();

    tokio::spawn(async move {
        let data = if use_parquet {
            if let Some(ref parquet_dir) = parquet_data_dir {
                info!(dir = %parquet_dir.display(), "loading backtest data from parquet");
                match crate::data::load_parquet(parquet_dir, Some((start_time, end_time))) {
                    Ok(d) => Arc::new(d),
                    Err(e) => {
                        let message = format!("failed to load parquet data: {}", e);
                        error!(error = %e, "backtest parquet load failed");
                        mark_backtest_failed(
                            &run_store,
                            &backtest_state,
                            &session_state,
                            &run_id_for_task,
                            message,
                        )
                        .await;
                        return;
                    }
                }
            } else {
                mark_backtest_failed(
                    &run_store,
                    &backtest_state,
                    &session_state,
                    &run_id_for_task,
                    "parquet data source requested but no parquet_data_dir configured".into(),
                )
                .await;
                return;
            }
        } else {
            info!("loading backtest data from historical sqlite");
            match HistoricalData::load_sqlite(&historical_store, start_time, end_time).await {
                Ok(d) => Arc::new(d),
                Err(e) => {
                    let message = format!("failed to load data from sqlite: {}", e);
                    error!(error = %e, "backtest sqlite load failed");
                    mark_backtest_failed(
                        &run_store,
                        &backtest_state,
                        &session_state,
                        &run_id_for_task,
                        message,
                    )
                    .await;
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
        let result_json = match serde_json::to_value(&result) {
            Ok(value) => value,
            Err(e) => {
                error!(run_id = %run_id_for_task, error = %e, "failed to serialize backtest result");
                serde_json::Value::Null
            }
        };

        if let Err(e) = run_store
            .complete_backtest_run(&run_id_for_task, &result_json)
            .await
        {
            error!(run_id = %run_id_for_task, error = %e, "failed to persist completed backtest run");
        }

        let mut guard = backtest_state.lock().await;
        guard.status = BacktestRunStatus::Complete;
        guard.result = Some(result);

        finish_backtest_session(&run_store, &session_state, "complete", None).await;
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
        run_id: guard.run_id.clone(),
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

#[derive(Deserialize)]
pub struct BacktestRunsQuery {
    #[serde(default = "default_backtest_runs_limit")]
    pub limit: u32,
}

fn default_backtest_runs_limit() -> u32 {
    25
}

pub async fn get_backtest_runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BacktestRunsQuery>,
) -> Result<Json<Vec<BacktestRunRecord>>, StatusCode> {
    match state.store.get_recent_backtest_runs(query.limit).await {
        Ok(mut runs) => {
            for run in &mut runs {
                run.result = None;
            }
            Ok(Json(runs))
        }
        Err(e) => {
            error!(error = %e, "failed to get backtest runs");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_backtest_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<BacktestRunRecord>, StatusCode> {
    match state.store.get_backtest_run(&id).await {
        Ok(Some(run)) => Ok(Json(run)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!(id = %id, error = %e, "failed to get backtest run");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn post_backtest_stop(State(state): State<Arc<AppState>>) -> StatusCode {
    let stopped_run_id = {
        let mut guard = state.backtest.lock().await;
        let was_running = matches!(guard.status, BacktestRunStatus::Running { .. });
        let run_id = guard.run_id.take();
        // Reset backtest state
        guard.status = BacktestRunStatus::Idle;
        guard.error = None;
        guard.progress = None;
        guard.live_snapshot = None;
        // Keep result if there was one. Only a running job should rewrite durable history as stopped.
        if was_running {
            run_id
        } else {
            None
        }
    };

    if let Some(run_id) = stopped_run_id {
        if let Err(e) = state
            .store
            .finish_backtest_run_with_error(&run_id, "stopped", "backtest stopped/reset via API")
            .await
        {
            error!(run_id = %run_id, error = %e, "failed to persist stopped backtest run");
        }
    }

    finish_backtest_session(
        &state.store,
        &state.session,
        "stopped",
        Some("backtest stopped/reset via API"),
    )
    .await;

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

#[derive(Deserialize)]
pub struct SessionsQuery {
    #[serde(default = "default_sessions_limit")]
    pub limit: u32,
}

fn default_sessions_limit() -> u32 {
    25
}

fn session_error(
    status: StatusCode,
    error: impl Into<String>,
) -> (StatusCode, Json<SessionErrorResponse>) {
    (
        status,
        Json(SessionErrorResponse {
            error: error.into(),
        }),
    )
}

fn session_config_value(
    config: Option<&SessionConfig>,
) -> Result<Option<serde_json::Value>, (StatusCode, Json<SessionErrorResponse>)> {
    config
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| session_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn persist_session_started(
    state: &Arc<AppState>,
    session: &super::SessionState,
) -> Result<(), (StatusCode, Json<SessionErrorResponse>)> {
    state
        .store
        .record_session_started(&NewSessionRun {
            session_id: session.session_id.clone(),
            mode: session.mode.to_string(),
            started_at: session.started_at.clone().unwrap_or_else(Utc::now),
            config: session_config_value(session.config.as_ref())?,
        })
        .await
        .map(|_| ())
        .map_err(|e| {
            error!(session_id = %session.session_id, error = %e, "failed to persist session start");
            session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to persist session start",
            )
        })
}

async fn finish_session_record(
    state: &Arc<AppState>,
    session_id: &str,
    status: &str,
    reason: Option<&str>,
) {
    if let Err(e) = state
        .store
        .finish_session_run(session_id, status, reason)
        .await
    {
        error!(session_id = %session_id, status = %status, error = %e, "failed to persist session end");
    }
}

pub async fn get_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SessionsQuery>,
) -> Result<Json<Vec<SessionRunRecord>>, StatusCode> {
    state
        .store
        .get_recent_session_runs(query.limit)
        .await
        .map(Json)
        .map_err(|e| {
            error!(error = %e, "failed to get session runs");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn get_session_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SessionRunRecord>, StatusCode> {
    match state.store.get_session_run(&id).await {
        Ok(Some(run)) => Ok(Json(run)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!(id = %id, error = %e, "failed to get session run");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn post_session_start(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SessionStartRequest>,
) -> Result<StatusCode, (StatusCode, Json<SessionErrorResponse>)> {
    {
        let session = state.session.read().await;
        if session.trading_active {
            return Err(session_error(
                StatusCode::CONFLICT,
                "session already running",
            ));
        }
    }

    match req.mode {
        SessionMode::Paper => {
            let session = super::SessionState::new_session(SessionMode::Paper, req.config);
            persist_session_started(&state, &session).await?;
            {
                let mut active = state.session.write().await;
                *active = session;
            }

            state.engine.resume().await;
            info!("paper trading session started via API");
            Ok(StatusCode::OK)
        }
        SessionMode::Backtest => {
            let config = req.config;
            let start = config.backtest_start.clone().ok_or_else(|| {
                session_error(
                    StatusCode::BAD_REQUEST,
                    "backtest_start required for backtest mode",
                )
            })?;
            let end = config.backtest_end.clone().ok_or_else(|| {
                session_error(
                    StatusCode::BAD_REQUEST,
                    "backtest_end required for backtest mode",
                )
            })?;

            crate::parse_date(&start).map_err(|e| {
                session_error(
                    StatusCode::BAD_REQUEST,
                    format!("invalid backtest_start: {e}"),
                )
            })?;
            crate::parse_date(&end).map_err(|e| {
                session_error(
                    StatusCode::BAD_REQUEST,
                    format!("invalid backtest_end: {e}"),
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
                data_source: None,
            };

            let session = super::SessionState::new_session(SessionMode::Backtest, config);
            let session_id = session.session_id.clone();
            persist_session_started(&state, &session).await?;
            {
                let mut active = state.session.write().await;
                *active = session;
            }

            let state_for_backtest = state.clone();
            if let Err((code, Json(e))) =
                post_backtest_run(State(state_for_backtest), Json(backtest_req)).await
            {
                {
                    let mut active = state.session.write().await;
                    if active.session_id == session_id {
                        *active = super::SessionState::default();
                    }
                }
                finish_session_record(&state, &session_id, "failed", Some(&e.error)).await;
                return Err((code, Json(SessionErrorResponse { error: e.error })));
            }

            Ok(StatusCode::OK)
        }
        SessionMode::Live => Err(session_error(
            StatusCode::NOT_IMPLEMENTED,
            "live trading not yet implemented",
        )),
        SessionMode::Idle => {
            let stopped_session_id = {
                let mut session = state.session.write().await;
                let id = (!session.session_id.is_empty()).then(|| session.session_id.clone());
                *session = super::SessionState::default();
                id
            };
            if let Some(session_id) = stopped_session_id {
                finish_session_record(
                    &state,
                    &session_id,
                    "stopped",
                    Some("session set idle via API"),
                )
                .await;
            }
            Ok(StatusCode::OK)
        }
    }
}

pub async fn post_session_stop(State(state): State<Arc<AppState>>) -> StatusCode {
    let stopped_session_id = {
        let mut session = state.session.write().await;
        let id = (!session.session_id.is_empty()).then(|| session.session_id.clone());
        *session = super::SessionState::default();
        id
    };

    if let Some(session_id) = stopped_session_id {
        finish_session_record(
            &state,
            &session_id,
            "stopped",
            Some("session stopped via API"),
        )
        .await;
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
    let session_id = {
        let mut session = state.session.write().await;
        session.config = Some(config.clone());
        (!session.session_id.is_empty()).then(|| session.session_id.clone())
    };

    if let Some(session_id) = session_id {
        match serde_json::to_value(&config) {
            Ok(config_value) => {
                if let Err(e) = state
                    .store
                    .update_session_config(&session_id, Some(&config_value))
                    .await
                {
                    error!(session_id = %session_id, error = %e, "failed to persist session config update");
                }
            }
            Err(e) => {
                error!(session_id = %session_id, error = %e, "failed to serialize session config update")
            }
        }
    }

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
    Query(query): Query<MarketsQuery>,
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

#[derive(Deserialize)]
pub struct DecisionsQuery {
    #[serde(default = "default_decision_limit")]
    pub limit: u32,
}

fn default_decision_limit() -> u32 {
    100
}

pub async fn get_decisions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DecisionsQuery>,
) -> Result<Json<Vec<DecisionRecord>>, StatusCode> {
    state
        .store
        .get_recent_decisions(query.limit)
        .await
        .map(Json)
        .map_err(|e| {
            error!(error = %e, "failed to get recent decisions");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn get_decision(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<DecisionRecord>, StatusCode> {
    match state.store.get_decision(id).await {
        Ok(Some(decision)) => Ok(Json(decision)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!(id, error = %e, "failed to get decision");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_market_decisions(
    State(state): State<Arc<AppState>>,
    Path(ticker): Path<String>,
    Query(query): Query<DecisionsQuery>,
) -> Result<Json<Vec<DecisionRecord>>, StatusCode> {
    state
        .store
        .get_decisions_for_ticker(&ticker, query.limit)
        .await
        .map(Json)
        .map_err(|e| {
            error!(ticker = %ticker, error = %e, "failed to get decisions for ticker");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[derive(Deserialize)]
pub struct AuditQuery {
    #[serde(default = "default_audit_limit")]
    pub limit: u32,
}

fn default_audit_limit() -> u32 {
    100
}

#[derive(Serialize)]
pub struct AuditEventCreated {
    pub id: i64,
}

pub async fn get_audit_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<AuditEvent>>, StatusCode> {
    state
        .store
        .get_recent_audit_events(query.limit)
        .await
        .map(Json)
        .map_err(|e| {
            error!(error = %e, "failed to get audit events");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn post_audit_event(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(mut event): Json<NewAuditEvent>,
) -> Result<Json<AuditEventCreated>, StatusCode> {
    if event.trace_id.is_none() {
        event.trace_id = headers
            .get(super::TRACE_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
    }

    state
        .store
        .record_audit_event(&event)
        .await
        .map(|id| Json(AuditEventCreated { id }))
        .map_err(|e| {
            error!(error = %e, "failed to record audit event");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
