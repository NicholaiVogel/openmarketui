//! Paper trading engine implementation

use super::state::EngineState;
use crate::api::KalshiClient;
use crate::config::AppConfig;
use crate::pipeline::TradingPipeline;
use crate::sources::{LiveKalshiSource, PaperExecutor};
use chrono::Utc;
use pm_core::{
    Filter, OrderExecutor, Portfolio, Scorer, Selector, Trade, TradeType, TradingContext,
};
use pm_engine::{CbCheckContext, CbStatus, CircuitBreakerState};
use pm_garden::{
    AlreadyPositionedFilter, BollingerMeanReversionScorer, CategoryWeightedScorer,
    MeanReversionScorer, MomentumScorer, MultiTimeframeMomentumScorer, OrderFlowScorer,
    TimeDecayScorer, TimeToCloseFilter, VolumeScorer,
};
use pm_store::SqliteStore;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{debug, error, info, warn};

pub struct EngineStatus {
    pub state: EngineState,
    pub uptime_secs: u64,
    pub last_tick: Option<chrono::DateTime<Utc>>,
    pub ticks_completed: u64,
}

#[derive(Debug, Clone)]
pub struct DecisionInfo {
    pub ticker: String,
    pub action: String, // "enter", "exit", "skip"
    pub side: Option<String>,
    pub score: f64,
    pub scorer_breakdown: HashMap<String, f64>,
    pub reason: Option<String>,
    pub latency_ms: u64,
    pub timestamp: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TickMetrics {
    pub candidates_fetched: usize,
    pub candidates_filtered: usize,
    pub candidates_selected: usize,
    pub signals_generated: usize,
    pub fills_executed: usize,
    pub duration_ms: u64,
    pub decisions: Vec<DecisionInfo>,
}

pub struct PaperTradingEngine {
    config: AppConfig,
    store: Arc<SqliteStore>,
    executor: Arc<PaperExecutor>,
    pipeline: Mutex<TradingPipeline>,
    circuit_breaker: Mutex<CircuitBreakerState>,
    state: RwLock<EngineState>,
    context: RwLock<TradingContext>,
    shutdown_tx: broadcast::Sender<()>,
    tick_tx: broadcast::Sender<TickMetrics>,
    start_time: Instant,
    ticks: RwLock<u64>,
    last_tick: RwLock<Option<chrono::DateTime<Utc>>>,
    last_candidates: RwLock<Vec<pm_core::MarketCandidate>>,
}

impl PaperTradingEngine {
    pub async fn new(
        config: AppConfig,
        store: Arc<SqliteStore>,
        executor: Arc<PaperExecutor>,
        client: Arc<KalshiClient>,
    ) -> anyhow::Result<Self> {
        let (shutdown_tx, _) = broadcast::channel(1);
        let (tick_tx, _) = broadcast::channel(64);

        let pipeline = Self::build_pipeline(client, &config);

        let cb_config = config.circuit_breaker.clone().into();
        let circuit_breaker = CircuitBreakerState::new(cb_config);

        let initial_capital =
            Decimal::try_from(config.trading.initial_capital).unwrap_or(Decimal::new(10000, 0));

        let portfolio = store
            .load_portfolio()
            .await?
            .unwrap_or_else(|| Portfolio::new(initial_capital));

        let mut ctx = TradingContext::new(portfolio.initial_capital, Utc::now());
        ctx.portfolio = portfolio;

        Ok(Self {
            config,
            store,
            executor,
            pipeline: Mutex::new(pipeline),
            circuit_breaker: Mutex::new(circuit_breaker),
            state: RwLock::new(EngineState::Starting),
            context: RwLock::new(ctx),
            shutdown_tx,
            tick_tx,
            start_time: Instant::now(),
            ticks: RwLock::new(0),
            last_tick: RwLock::new(None),
            last_candidates: RwLock::new(Vec::new()),
        })
    }

    fn build_pipeline(client: Arc<KalshiClient>, config: &AppConfig) -> TradingPipeline {
        let sources: Vec<Box<dyn pm_core::Source>> = vec![Box::new(LiveKalshiSource::new(client))];

        let max_pos_size =
            (config.trading.initial_capital * config.trading.max_position_pct) as u64;

        let filters: Vec<Box<dyn Filter>> = vec![
            Box::new(TimeToCloseFilter::new(
                config.trading.min_time_to_close_hours,
                Some(config.trading.max_time_to_close_hours),
            )),
            Box::new(AlreadyPositionedFilter::new(max_pos_size.max(100))),
        ];

        let scorers: Vec<Box<dyn Scorer>> = vec![
            Box::new(MomentumScorer::new(6)),
            Box::new(MultiTimeframeMomentumScorer::default_windows()),
            Box::new(MeanReversionScorer::new(24)),
            Box::new(BollingerMeanReversionScorer::default_config()),
            Box::new(VolumeScorer::new(6)),
            Box::new(OrderFlowScorer::new()),
            Box::new(TimeDecayScorer::new()),
            Box::new(CategoryWeightedScorer::with_defaults()),
        ];

        let max_positions = config.trading.max_positions;
        let selector: Box<dyn Selector> =
            Box::new(crate::pipeline::TopKSelector::new(max_positions));

        TradingPipeline::new(sources, filters, scorers, selector, max_positions)
    }

    pub fn shutdown_handle(&self) -> broadcast::Sender<()> {
        self.shutdown_tx.clone()
    }

    pub fn subscribe_ticks(&self) -> broadcast::Receiver<TickMetrics> {
        self.tick_tx.subscribe()
    }

    pub async fn get_status(&self) -> EngineStatus {
        EngineStatus {
            state: self.state.read().await.clone(),
            uptime_secs: self.start_time.elapsed().as_secs(),
            last_tick: *self.last_tick.read().await,
            ticks_completed: *self.ticks.read().await,
        }
    }

    pub async fn get_context(&self) -> TradingContext {
        self.context.read().await.clone()
    }

    pub async fn get_current_prices(&self) -> HashMap<String, Decimal> {
        self.executor.get_current_prices().await
    }

    pub async fn get_last_candidates(&self) -> Vec<pm_core::MarketCandidate> {
        self.last_candidates.read().await.clone()
    }

    pub async fn pause(&self, reason: String) {
        let mut state = self.state.write().await;
        *state = EngineState::Paused(reason);
    }

    pub async fn resume(&self) {
        let mut state = self.state.write().await;
        if matches!(*state, EngineState::Paused(_)) {
            *state = EngineState::Running;
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let poll_interval = std::time::Duration::from_secs(self.config.kalshi.poll_interval_secs);

        {
            let mut state = self.state.write().await;
            *state = EngineState::Recovering;
        }

        info!("recovering state from SQLite");
        if let Ok(Some(portfolio)) = self.store.load_portfolio().await {
            let mut ctx = self.context.write().await;
            ctx.portfolio = portfolio;
            info!(
                positions = ctx.portfolio.positions.len(),
                cash = %ctx.portfolio.cash,
                "state recovered"
            );
        }

        {
            let mut state = self.state.write().await;
            *state = EngineState::Running;
        }

        info!(
            interval_secs = self.config.kalshi.poll_interval_secs,
            "engine running"
        );

        self.tick().await;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("shutdown signal received");
                    let mut state = self.state.write().await;
                    *state = EngineState::ShuttingDown;
                    break;
                }
                _ = tokio::time::sleep(poll_interval) => {
                    let current_state = self.state.read().await.clone();
                    match current_state {
                        EngineState::Running => {
                            self.tick().await;
                        }
                        EngineState::Paused(ref reason) => {
                            info!(reason = %reason, "engine paused, skipping tick");
                        }
                        _ => {}
                    }
                }
            }
        }

        info!("persisting final state");
        let ctx = self.context.read().await;
        if let Err(e) = self.store.save_portfolio(&ctx.portfolio).await {
            error!(error = %e, "failed to persist final state");
        }

        info!("engine shutdown complete");
        Ok(())
    }

    async fn tick(&self) {
        let tick_start = Instant::now();
        let now = Utc::now();

        let context_snapshot = {
            let mut ctx = self.context.write().await;
            ctx.timestamp = now;
            ctx.request_id = uuid::Uuid::new_v4().to_string();
            ctx.clone()
        };

        let result = {
            let pipeline = self.pipeline.lock().await;
            pipeline.execute(context_snapshot.clone()).await
        };

        let candidates_fetched = result.retrieved_candidates.len();
        let candidates_filtered = result.filtered_candidates.len();
        let candidates_selected = result.selected_candidates.len();

        // Store candidates for API access
        {
            let mut last = self.last_candidates.write().await;
            *last = result.retrieved_candidates.clone();
        }

        let candidate_scores: HashMap<String, f64> = result
            .selected_candidates
            .iter()
            .map(|c| (c.ticker.clone(), c.final_score))
            .collect();

        self.executor
            .update_market_state(&result.retrieved_candidates)
            .await;

        let exit_signals = self
            .executor
            .generate_exit_signals(&context_snapshot, &candidate_scores);

        if !exit_signals.is_empty() {
            info!(count = exit_signals.len(), "exit signals generated");
            for exit in &exit_signals {
                debug!(
                    ticker = %exit.ticker,
                    reason = ?exit.reason,
                    price = %exit.current_price,
                    "exit signal"
                );
            }
        }

        let mut ctx = self.context.write().await;
        let mut fills_executed = 0u32;
        let mut exits_executed = 0u32;
        let mut decisions: Vec<DecisionInfo> = Vec::new();

        for exit in &exit_signals {
            if let Some(position) = ctx.portfolio.positions.get(&exit.ticker).cloned() {
                let exit_exec_start = Instant::now();
                let maybe_fill = self
                    .executor
                    .execute_exit_fill(
                        &exit.ticker,
                        position.side,
                        position.quantity,
                        now,
                        Some(exit.current_price),
                    )
                    .await;

                let Some(exit_fill) = maybe_fill else {
                    debug!(ticker = %exit.ticker, "exit signal not filled");
                    continue;
                };

                let pnl = ctx.portfolio.close_position_partial(
                    &exit.ticker,
                    exit_fill.quantity,
                    exit_fill.price,
                    exit_fill.fee,
                );

                info!(
                    ticker = %exit.ticker,
                    reason = ?exit.reason,
                    qty = exit_fill.quantity,
                    fill_price = %exit_fill.price,
                    fee = ?exit_fill.fee,
                    pnl = ?pnl,
                    "paper exit"
                );

                let reason_str = format!("{:?}", exit.reason);
                let _ = self
                    .store
                    .record_fill(&exit_fill, pnl, Some(&reason_str))
                    .await;

                ctx.trading_history.push(Trade {
                    ticker: exit.ticker.clone(),
                    side: position.side,
                    quantity: exit_fill.quantity,
                    price: exit_fill.price,
                    timestamp: exit_fill.timestamp,
                    trade_type: TradeType::Close,
                });

                // record exit decision
                decisions.push(DecisionInfo {
                    ticker: exit.ticker.clone(),
                    action: "exit".to_string(),
                    side: Some(format!("{:?}", position.side)),
                    score: candidate_scores.get(&exit.ticker).copied().unwrap_or(0.0),
                    scorer_breakdown: HashMap::new(),
                    reason: Some(reason_str),
                    latency_ms: exit_exec_start.elapsed().as_millis() as u64,
                    timestamp: now,
                });

                fills_executed += 1;
                exits_executed += 1;
            }
        }

        if exits_executed > 0 {
            info!(
                exits = exits_executed,
                cash = %ctx.portfolio.cash,
                "exits completed"
            );
        }

        let signals = self
            .executor
            .generate_signals(&result.selected_candidates, &*ctx);
        let signals_generated = signals.len();

        let market_metadata: HashMap<String, (&str, &str, chrono::DateTime<Utc>)> = result
            .selected_candidates
            .iter()
            .map(|c| {
                (
                    c.ticker.clone(),
                    (c.title.as_str(), c.category.as_str(), c.close_time),
                )
            })
            .collect();

        if signals_generated > 0 {
            info!(count = signals_generated, "entry signals generated");
            for sig in &signals {
                debug!(
                    ticker = %sig.ticker,
                    side = ?sig.side,
                    qty = sig.quantity,
                    limit = ?sig.limit_price,
                    reason = %sig.reason,
                    "entry signal"
                );
            }
        }

        let peak_equity = self
            .store
            .get_peak_equity()
            .await
            .ok()
            .flatten()
            .unwrap_or(ctx.portfolio.initial_capital);

        let positions_value: Decimal = ctx
            .portfolio
            .positions
            .values()
            .map(|p| p.avg_entry_price * Decimal::from(p.quantity))
            .sum();
        let current_equity = ctx.portfolio.cash + positions_value;

        let daily_pnl = self.calculate_daily_pnl().await;
        let hourly_fills = self.count_recent_fills(1).await;
        let daily_fills = self.count_recent_fills(24).await;

        let cb_ctx = CbCheckContext {
            current_equity,
            peak_equity,
            positions_count: ctx.portfolio.positions.len(),
            daily_pnl,
            hourly_fills,
            daily_fills,
        };

        let cb_status = {
            let cb = self.circuit_breaker.lock().await;
            cb.check(&cb_ctx)
        };

        if let CbStatus::Tripped(reason) = cb_status {
            warn!(reason = %reason, "circuit breaker tripped, pausing");
            drop(ctx);
            let mut state = self.state.write().await;
            *state = EngineState::Paused(reason);
            return;
        }

        {
            let mut cb = self.circuit_breaker.lock().await;
            cb.record_success();
        }

        let mut entries_this_tick = 0usize;
        let max_entries = self.config.trading.max_entries_per_tick;
        let cash_reserve = Decimal::try_from(
            self.config.trading.initial_capital * self.config.trading.cash_reserve_pct,
        )
        .unwrap_or(Decimal::ZERO);

        for signal in signals {
            if ctx.portfolio.positions.len() >= self.config.trading.max_positions {
                break;
            }

            if entries_this_tick >= max_entries {
                debug!(
                    max = max_entries,
                    "max entries per tick reached, deferring remaining"
                );
                break;
            }

            if ctx.portfolio.cash <= cash_reserve {
                debug!(
                    cash = %ctx.portfolio.cash,
                    reserve = %cash_reserve,
                    "cash reserve reached, skipping entry"
                );
                break;
            }

            let context_for_exec = (*ctx).clone();
            let entry_exec_start = Instant::now();
            if let Some(fill) = self
                .executor
                .execute_signal(&signal, &context_for_exec)
                .await
            {
                info!(
                    ticker = %fill.ticker,
                    side = ?fill.side,
                    qty = fill.quantity,
                    price = %fill.price,
                    "paper fill"
                );

                if let Some((title, category, close_time)) = market_metadata.get(&fill.ticker) {
                    ctx.portfolio.apply_fill_with_metadata(
                        &fill,
                        Some(title),
                        Some(category),
                        Some(*close_time),
                    );
                } else {
                    ctx.portfolio.apply_fill(&fill);
                }
                ctx.trading_history.push(Trade {
                    ticker: fill.ticker.clone(),
                    side: fill.side,
                    quantity: fill.quantity,
                    price: fill.price,
                    timestamp: fill.timestamp,
                    trade_type: TradeType::Open,
                });

                // record entry decision
                let candidate_score = candidate_scores.get(&signal.ticker).copied().unwrap_or(0.0);
                decisions.push(DecisionInfo {
                    ticker: signal.ticker.clone(),
                    action: "enter".to_string(),
                    side: Some(format!("{:?}", signal.side)),
                    score: candidate_score,
                    scorer_breakdown: HashMap::new(),
                    reason: Some(signal.reason.clone()),
                    latency_ms: entry_exec_start.elapsed().as_millis() as u64,
                    timestamp: now,
                });

                fills_executed += 1;
                entries_this_tick += 1;
            }
        }

        if let Err(e) = self.store.save_portfolio(&ctx.portfolio).await {
            error!(error = %e, "failed to persist portfolio");
            let mut cb = self.circuit_breaker.lock().await;
            cb.record_error();
        }

        let positions_value: Decimal = ctx
            .portfolio
            .positions
            .values()
            .map(|p| p.avg_entry_price * Decimal::from(p.quantity))
            .sum();
        let equity = ctx.portfolio.cash + positions_value;
        let drawdown = if peak_equity > Decimal::ZERO {
            ((peak_equity - equity).to_f64().unwrap_or(0.0)) / peak_equity.to_f64().unwrap_or(1.0)
        } else {
            0.0
        };

        let _ = self
            .store
            .snapshot_equity(
                now,
                equity,
                ctx.portfolio.cash,
                positions_value,
                drawdown.max(0.0),
            )
            .await;

        let duration_ms = tick_start.elapsed().as_millis() as u64;

        let _ = self
            .store
            .record_pipeline_run(
                now,
                duration_ms,
                candidates_fetched,
                candidates_filtered,
                candidates_selected,
                signals_generated,
                fills_executed as usize,
                None,
            )
            .await;

        {
            let mut ticks = self.ticks.write().await;
            *ticks += 1;
        }
        {
            let mut last = self.last_tick.write().await;
            *last = Some(now);
        }

        let positions_count = ctx.portfolio.positions.len();
        let return_pct = if ctx.portfolio.initial_capital > Decimal::ZERO {
            ((equity - ctx.portfolio.initial_capital) * Decimal::from(100))
                / ctx.portfolio.initial_capital
        } else {
            Decimal::ZERO
        };

        let next_tick =
            now + chrono::Duration::seconds(self.config.kalshi.poll_interval_secs as i64);

        info!(
            positions = positions_count,
            exits = exits_executed,
            entries = fills_executed - exits_executed,
            equity = %equity,
            return_pct = %return_pct.round_dp(2),
            duration_ms = duration_ms,
            next_tick = %next_tick.format("%H:%M:%S"),
            "tick complete"
        );

        let _ = self.tick_tx.send(TickMetrics {
            candidates_fetched,
            candidates_filtered,
            candidates_selected,
            signals_generated,
            fills_executed: fills_executed as usize,
            duration_ms,
            decisions,
        });
    }

    async fn calculate_daily_pnl(&self) -> f64 {
        let fills = self.store.get_recent_fills(1000).await.unwrap_or_default();
        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0);
        let today_utc = today_start.map(|ts| chrono::TimeZone::from_utc_datetime(&Utc, &ts));

        fills
            .iter()
            .filter(|f| today_utc.map_or(false, |t| f.timestamp >= t))
            .filter_map(|f| f.pnl.as_ref()?.to_f64())
            .sum()
    }

    async fn count_recent_fills(&self, hours: i64) -> u32 {
        let since = Utc::now() - chrono::Duration::hours(hours);
        self.store.get_fills_since(since).await.unwrap_or(0)
    }
}
